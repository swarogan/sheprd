//! What a site's card says.
use super::*;
use crate::menu::jira::status_line;
use sheprd_orchestrator::jira::{self, account::Account};
use std::sync::Arc;

fn account(email: Option<&str>) -> Account {
    Account {
        account_id: "5b10ac8d".into(),
        base_url: "https://acme.atlassian.net".into(),
        email: email.map(str::to_owned),
        host: "acme.atlassian.net".into(),
        name: "Ada Lovelace".into(),
    }
}

#[test]
fn a_card_says_whom_the_sign_in_is_or_why_it_is_not() {
    let line = |lookup| plain(status_line(&status(lookup)));
    assert_eq!(line(Lookup::Unknown), "Checking the sign-in\u{2026}");
    assert_eq!(
        line(Lookup::Signed(account(Some("ada@work.example")))),
        "Signed in as Ada Lovelace (ada@work.example)."
    );
    // Jira may hide the account's email: the saved one stands in.
    assert_eq!(
        line(Lookup::Signed(account(None))),
        "Signed in as Ada Lovelace (ada@example.com)."
    );
    assert_eq!(
        line(Lookup::Failed(Arc::new(jira::Error::Authentication))),
        "Jira refused the saved email or API token. Enter them again."
    );
    // Any other failure is said as it is, so a wrong address shows.
    assert_eq!(
        line(Lookup::Failed(Arc::new(jira::Error::NotFound {
            origin: "https://acme.atlassian.net".into()
        }))),
        "https://acme.atlassian.net does not answer as a Jira Cloud site; check the address"
    );
}
