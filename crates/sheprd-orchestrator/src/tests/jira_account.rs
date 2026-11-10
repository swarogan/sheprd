//! Checking a pasted site and sign-in, and the transport every Jira read
//! shares, against a local server standing in for the site. The saved sites
//! are `https` only, so these reach the server through the crate-internal
//! entry points that take an address as given.
use crate::jira::{self, Error as JiraError, Target, account};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    time::Duration,
};

/// A reply: status, extra headers, body.
type Answer = (u16, Vec<(&'static str, &'static str)>, String);

/// Answers each connection with the next of `answers`, returning each
/// request as read: its head with the headers, a blank line, and its body.
fn serve(answers: Vec<Answer>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        answers
            .into_iter()
            .map(|(status, headers, body)| {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    request.push_str(&line);
                    if line.trim_end().is_empty() {
                        break;
                    }
                }
                let mut payload = vec![0; length];
                reader.read_exact(&mut payload).unwrap();
                request.push_str(&String::from_utf8(payload).unwrap());
                let extra: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
                (&stream)
                    .write_all(
                        format!(
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                request
            })
            .collect()
    });
    (base, server)
}

/// The value of header `name` in a request as `serve` returns it.
fn header(request: &str, name: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

fn ok(body: Value) -> Answer {
    (200, Vec::new(), body.to_string())
}

fn token() -> SecretString {
    SecretString::from("s3cret")
}

fn check(base: &str, email: &str) -> Result<account::Account, JiraError> {
    account::check_at("127.0.0.1".into(), base.to_owned(), email, &token())
}

#[test]
fn the_basic_header_is_the_email_and_token_encoded_and_kept_out_of_logs() {
    // base64 of `me@example.com:s3cret`.
    let header = jira::authorization("me@example.com", &token()).unwrap();
    assert_eq!(
        header.to_str().unwrap(),
        "Basic bWVAZXhhbXBsZS5jb206czNjcmV0"
    );
    assert!(header.is_sensitive());
}

#[test]
fn a_site_and_sign_in_are_checked_with_the_myself_endpoint() {
    let body = json!({
        "accountId": "5b10ac8d82e05b22cc7d4ef5", "displayName": "Ada Lovelace",
        "emailAddress": "ada@example.com", "active": true,
    });
    let (base, server) = serve(vec![ok(body)]);
    let found = check(&base, " me@example.com ").unwrap();
    assert_eq!(found.account_id, "5b10ac8d82e05b22cc7d4ef5");
    assert_eq!(found.name, "Ada Lovelace");
    assert_eq!(found.email.as_deref(), Some("ada@example.com"));
    assert_eq!(
        (found.host.as_str(), found.base_url.as_str()),
        ("127.0.0.1", base.as_str())
    );
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("GET /rest/api/3/myself "),
        "{requests:?}"
    );
    assert_eq!(
        header(&requests[0], "authorization").as_deref(),
        Some("Basic bWVAZXhhbXBsZS5jb206czNjcmV0")
    );
}

#[test]
fn the_email_stands_in_for_a_missing_name_and_an_answer_without_an_account_is_a_refusal() {
    let (base, server) = serve(vec![ok(json!({ "accountId": "abc", "displayName": " " }))]);
    let found = check(&base, "me@example.com").unwrap();
    assert_eq!((found.name.as_str(), found.email), ("me@example.com", None));
    server.join().unwrap();

    for body in [
        json!({}),
        json!({ "accountId": "" }),
        json!({ "displayName": "Ada" }),
    ] {
        let (base, server) = serve(vec![ok(body)]);
        assert!(matches!(
            check(&base, "me@example.com"),
            Err(JiraError::Authentication)
        ));
        server.join().unwrap();
    }
    let (base, server) = serve(vec![(200, Vec::new(), "<html>sign in</html>".to_owned())]);
    assert!(matches!(
        check(&base, "me@example.com"),
        Err(JiraError::Json(_))
    ));
    server.join().unwrap();
}

#[test]
fn a_refusal_is_mapped_by_its_status() {
    for (status, expect) in [
        (401, "Authentication"),
        (403, "Authentication"),
        (404, "NotFound"),
        (429, "RateLimit"),
        (500, "Status(500)"),
    ] {
        let (base, server) = serve(vec![(status, Vec::new(), "{}".to_owned())]);
        let error = check(&base, "me@example.com").unwrap_err();
        let shown = match &error {
            JiraError::NotFound { origin } => {
                assert_eq!(origin, &base);
                "NotFound".to_owned()
            }
            other => format!("{other:?}"),
        };
        assert_eq!(shown, expect);
        server.join().unwrap();
    }
}

#[test]
fn a_rate_limit_waits_as_long_as_jira_asked_within_bounds() {
    let wait = |headers: Vec<(&'static str, &'static str)>| {
        let (base, server) = serve(vec![(429, headers, "{}".to_owned())]);
        let mut cooldown = None;
        let target = Target {
            base_url: &base,
            email: "me@example.com",
            token: &token(),
        };
        let url = jira::api(&base, &["myself"]).unwrap();
        let error = jira::send(
            &jira::agent(Duration::from_secs(5)),
            &target,
            &url,
            None,
            1024,
            &mut cooldown,
        )
        .unwrap_err();
        assert!(matches!(error, JiraError::RateLimit), "{error}");
        server.join().unwrap();
        cooldown
    };
    assert_eq!(
        wait(vec![("Retry-After", "7200")]),
        Some(Duration::from_secs(7200))
    );
    // Too short to be worth asking again, too long to wait out, and not a number or absent.
    assert_eq!(
        wait(vec![("Retry-After", "1")]),
        Some(Duration::from_secs(300))
    );
    assert_eq!(
        wait(vec![("Retry-After", "999999")]),
        Some(Duration::from_secs(86400))
    );
    assert_eq!(
        wait(vec![("Retry-After", "Wed, 21 Oct 2026 07:28:00 GMT")]),
        Some(Duration::from_secs(3600))
    );
    assert_eq!(wait(vec![]), Some(Duration::from_secs(3600)));

    let none = ureq::http::HeaderMap::new();
    assert_eq!(jira::retry_after(401, &none), None);
    assert_eq!(jira::retry_after(500, &none), None);
}

#[test]
fn a_search_is_posted_as_json_with_the_basic_header() {
    let (base, server) = serve(vec![ok(json!({ "issues": [] }))]);
    let target = Target {
        base_url: &base,
        email: "me@example.com",
        token: &token(),
    };
    let url = jira::api(&base, &["search", "jql"]).unwrap();
    let body = json!({ "jql": "x", "maxResults": 100 });
    let reply = jira::send(
        &jira::agent(Duration::from_secs(5)),
        &target,
        &url,
        Some(&body),
        1024,
        &mut None,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&reply).unwrap(),
        json!({ "issues": [] })
    );
    let request = server.join().unwrap().remove(0);
    let (head, sent) = request.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("POST /rest/api/3/search/jql "), "{head}");
    assert_eq!(
        header(head, "content-type").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        header(head, "authorization").as_deref(),
        Some("Basic bWVAZXhhbXBsZS5jb206czNjcmV0")
    );
    assert_eq!(serde_json::from_str::<Value>(sent).unwrap(), body);
}

#[test]
fn a_redirect_is_not_followed_with_the_token_and_a_huge_answer_is_refused() {
    let (base, server) = serve(vec![(
        302,
        vec![("Location", "http://127.0.0.1:1/")],
        String::new(),
    )]);
    assert!(matches!(
        check(&base, "me@example.com"),
        Err(JiraError::Status(302))
    ));
    assert_eq!(server.join().unwrap().len(), 1);

    let (base, server) = serve(vec![(200, Vec::new(), "x".repeat(2048))]);
    let target = Target {
        base_url: &base,
        email: "me@example.com",
        token: &token(),
    };
    let url = jira::api(&base, &["myself"]).unwrap();
    let error = jira::send(
        &jira::agent(Duration::from_secs(5)),
        &target,
        &url,
        None,
        1024,
        &mut None,
    )
    .unwrap_err();
    assert!(matches!(error, JiraError::Size), "{error}");
    server.join().unwrap();
}

#[test]
fn a_bad_address_email_or_token_is_refused_before_any_request() {
    for address in [
        "http://acme.atlassian.net",
        "ftp://acme.atlassian.net",
        "https://me:pw@acme.atlassian.net",
        "",
    ] {
        assert!(
            matches!(
                account::check(address, "me@example.com", &token()),
                Err(JiraError::Address(_))
            ),
            "{address}"
        );
    }
    // Nothing listens here: a request would fail as a network error, not these.
    assert!(matches!(
        check("http://127.0.0.1:1", "nope"),
        Err(JiraError::Email)
    ));
    let spaced = SecretString::from("two words");
    assert!(matches!(
        account::check_at(
            "127.0.0.1".into(),
            "http://127.0.0.1:1".into(),
            "me@example.com",
            &spaced
        ),
        Err(JiraError::Token)
    ));
    assert!(matches!(
        check("http://127.0.0.1:1", "me@example.com"),
        Err(JiraError::Network { .. })
    ));
}
