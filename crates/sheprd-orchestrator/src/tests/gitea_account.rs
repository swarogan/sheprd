//! Checking a pasted Gitea address and token before they are saved, against a
//! local server standing in for the instance.
use super::gitlab_merge_requests::serve;
use crate::gitea::{self, account};
use secrecy::SecretString;

fn token() -> SecretString {
    SecretString::from("0123456789abcdef")
}

#[test]
fn an_address_and_token_are_checked_with_the_instances_user_endpoint() {
    let body = serde_json::json!({ "id": 1, "login": "alice", "full_name": "Alice Example" });
    let (address, server) = serve(vec![(200, body.to_string())]);
    let typed = format!("http://{address}/");
    let found = account::check(&typed, &token()).unwrap();
    assert_eq!(found.login, "alice");
    assert_eq!(found.name, "Alice Example");
    assert_eq!(found.base_url, format!("http://{address}"));
    assert_eq!(found.host, "127.0.0.1");
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /api/v1/user "), "{requests:?}");
}

#[test]
fn the_sign_in_name_stands_in_for_a_missing_full_name() {
    let (address, server) = serve(vec![(
        200,
        r#"{"login":"alice","full_name":" "}"#.to_owned(),
    )]);
    let found = account::check(&format!("http://{address}"), &token()).unwrap();
    assert_eq!(found.name, "alice");
    server.join().unwrap();
}

#[test]
fn a_refused_token_an_unknown_path_or_another_answer_is_not_saved() {
    let (address, server) = serve(vec![(401, "{}".to_owned())]);
    let error = account::check(&format!("http://{address}"), &token()).unwrap_err();
    assert!(matches!(error, gitea::Error::Authentication), "{error}");
    server.join().unwrap();

    // Gitea answers 403 for a token without the needed scope.
    let (address, server) = serve(vec![(403, "{}".to_owned())]);
    let error = account::check(&format!("http://{address}"), &token()).unwrap_err();
    assert!(matches!(error, gitea::Error::Authentication), "{error}");
    server.join().unwrap();

    // Not a Gitea address: the endpoint is not there.
    let (address, server) = serve(vec![(404, "not found".to_owned())]);
    let error = account::check(&format!("http://{address}"), &token()).unwrap_err();
    assert!(matches!(error, gitea::Error::NotGitea { .. }), "{error}");
    server.join().unwrap();

    // An answer that names no one is a refusal, not an account.
    let (address, server) = serve(vec![(200, "{}".to_owned())]);
    let error = account::check(&format!("http://{address}"), &token()).unwrap_err();
    assert!(matches!(error, gitea::Error::Authentication), "{error}");
    server.join().unwrap();
}

#[test]
fn a_redirect_is_not_followed_with_the_token() {
    let (address, server) = serve(vec![(302, String::new())]);
    let error = account::check(&format!("http://{address}"), &token()).unwrap_err();
    assert!(matches!(error, gitea::Error::Status(302)), "{error}");
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn a_bad_address_or_token_is_refused_before_any_request() {
    for address in [
        "ftp://gitea.arpa",
        "http://user:pw@gitea.arpa",
        "http://gitea.arpa/?x=1",
        "",
    ] {
        assert!(
            matches!(
                account::check(address, &token()),
                Err(gitea::Error::Address(_))
            ),
            "{address}"
        );
    }
    let error = account::check("http://127.0.0.1:1", &SecretString::from("two words")).unwrap_err();
    assert!(matches!(error, gitea::Error::Token), "{error}");
}
