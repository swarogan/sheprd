use super::*;
use sheprd_orchestrator::{Error, jira};

#[test]
fn every_jira_variant_has_a_message() {
    assert_localized(&[
        jira::Error::Address("http://acme.atlassian.net".into()),
        jira::Error::Authentication,
        jira::Error::Directory(sheprd_client::local_path::PathError::MissingHome),
        jira::Error::Email,
        jira::Error::EmailLength { limit: 254 },
        jira::Error::Json(serde_json::from_str::<u8>("x").unwrap_err()),
        jira::Error::Network {
            origin: "https://acme.atlassian.net".into(),
            source: Box::new(ureq::Error::HostNotFound),
        },
        jira::Error::NoStore(sheprd_credentials::Error::Policy),
        jira::Error::NotFound {
            origin: "https://acme.atlassian.net".into(),
        },
        jira::Error::RateLimit,
        jira::Error::Read(std::io::Error::other("pipe closed")),
        jira::Error::Record,
        jira::Error::RecordSize,
        jira::Error::SignIn {
            site: "acme.atlassian.net".into(),
        },
        jira::Error::Size,
        jira::Error::Status(500),
        jira::Error::Store(sheprd_credentials::Error::Permissions),
        jira::Error::Token,
        jira::Error::TokenLength { limit: 1024 },
        jira::Error::TooManySites,
    ]);
    #[cfg(target_os = "linux")]
    assert_localized(&[jira::Error::NoKeyring(
        sheprd_credentials::Error::SecretServiceUnavailable,
    )]);
}

#[test]
fn the_orchestrator_shows_a_jira_failure_as_its_own() {
    assert_localized(&[Error::Jira(jira::Error::RateLimit)]);
}
