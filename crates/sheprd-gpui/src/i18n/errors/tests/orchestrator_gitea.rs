use super::*;
use sheprd_orchestrator::{Error, gitea};

#[test]
fn every_gitea_variant_has_a_message() {
    let repository = || "ops/infra".to_owned();
    assert_localized(&[
        gitea::Error::Address("two words".into()),
        gitea::Error::Authentication,
        gitea::Error::Directory(sheprd_client::local_path::PathError::MissingHome),
        gitea::Error::Json(serde_json::from_str::<u8>("x").unwrap_err()),
        gitea::Error::Network {
            origin: "http://gitea.arpa".into(),
            source: Box::new(ureq::Error::HostNotFound),
        },
        gitea::Error::NoStore(sheprd_credentials::Error::Policy),
        gitea::Error::NotFound {
            repository: repository(),
        },
        gitea::Error::NotGitea {
            origin: "http://gitea.arpa".into(),
        },
        gitea::Error::RateLimit,
        gitea::Error::Read(std::io::Error::other("pipe closed")),
        gitea::Error::Record,
        gitea::Error::RecordSize,
        gitea::Error::SignIn {
            repository: repository(),
        },
        gitea::Error::Size,
        gitea::Error::Status(500),
        gitea::Error::Store(sheprd_credentials::Error::Permissions),
        gitea::Error::Token,
        gitea::Error::TokenLength { limit: 1024 },
        gitea::Error::TooManyInstances,
    ]);
    #[cfg(target_os = "linux")]
    assert_localized(&[gitea::Error::NoKeyring(
        sheprd_credentials::Error::SecretServiceUnavailable,
    )]);
}

#[test]
fn the_orchestrator_shows_a_gitea_failure_as_its_own() {
    assert_localized(&[Error::Gitea(gitea::Error::RateLimit)]);
}
