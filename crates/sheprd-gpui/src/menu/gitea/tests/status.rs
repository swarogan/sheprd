//! What an instance's card says, and when the address is an unencrypted one.
use super::*;
use crate::menu::gitea::{is_plain_http, status_line};
use sheprd_orchestrator::gitea::{self, account::Account};
use std::sync::Arc;

fn account() -> Account {
    Account {
        base_url: "http://gitea.arpa:3000".into(),
        host: "gitea.arpa".into(),
        login: "alice".into(),
        name: "Alice Example".into(),
    }
}

#[test]
fn a_card_says_whom_the_token_signs_in_as_or_why_it_does_not() {
    let line = |lookup| plain(status_line(&status(lookup)));
    assert_eq!(line(Lookup::Unknown), "Checking the token\u{2026}");
    assert_eq!(line(Lookup::Signed(account())), "Signed in as alice.");
    assert_eq!(
        line(Lookup::Failed(Arc::new(gitea::Error::Authentication))),
        "Gitea refused the saved token. Paste a new one."
    );
    // Any other failure is said as it is, so a wrong address shows.
    assert_eq!(
        line(Lookup::Failed(Arc::new(gitea::Error::NotGitea {
            origin: "http://gitea.arpa:3000".into()
        }))),
        "http://gitea.arpa:3000 does not answer as a Gitea instance; check the address and any sub-path"
    );
}

#[test]
fn only_an_http_address_travels_unencrypted() {
    for (typed, plain_http) in [
        ("http://gitea.arpa:3000", true),
        ("HTTP://Gitea.arpa", true),
        ("https://gitea.example.com", false),
        // No scheme is https.
        ("gitea.example.com", false),
        ("", false),
        ("two words", false),
    ] {
        assert_eq!(is_plain_http(typed), plain_http, "{typed:?}");
    }
}
