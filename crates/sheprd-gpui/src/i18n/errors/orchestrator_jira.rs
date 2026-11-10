//! Jira failures from `error-orchestrator-jira.ftl`. Network, parser, and
//! I/O text stays a placeable; a path or store failure is localized on its
//! own.

use super::Localize;
use crate::i18n::fl;
use sheprd_orchestrator::jira::{Error, saved::MAX_SITES};

impl Localize for Error {
    fn localize(&self) -> String {
        match self {
            Self::Address(address) => fl!(
                "error-orchestrator-jira-address",
                address = format!("{address:?}")
            ),
            Self::Authentication => fl!("error-orchestrator-jira-authentication"),
            Self::Directory(error) => fl!(
                "error-orchestrator-jira-directory",
                error = error.localize()
            ),
            Self::Email => fl!("error-orchestrator-jira-email"),
            Self::EmailLength { limit } => fl!(
                "error-orchestrator-jira-email-length",
                limit = limit.to_string()
            ),
            Self::Json(error) => fl!("error-orchestrator-jira-json", error = error.to_string()),
            Self::Network { origin, source } => fl!(
                "error-orchestrator-jira-network",
                origin = origin.as_str(),
                error = source.to_string()
            ),
            #[cfg(target_os = "linux")]
            Self::NoKeyring(_) => fl!("error-orchestrator-jira-no-keyring"),
            Self::NoStore(_) => fl!("error-orchestrator-jira-no-store"),
            Self::NotFound { origin } => {
                fl!(
                    "error-orchestrator-jira-not-found",
                    origin = origin.as_str()
                )
            }
            Self::RateLimit => fl!("error-orchestrator-jira-rate-limit"),
            Self::Read(error) => fl!("error-orchestrator-jira-read", error = error.to_string()),
            Self::Record => fl!("error-orchestrator-jira-record"),
            Self::RecordSize => fl!("error-orchestrator-jira-record-size"),
            Self::SignIn { site } => fl!("error-orchestrator-jira-sign-in", site = site.as_str()),
            Self::Size => fl!("error-orchestrator-jira-size"),
            Self::Status(status) => fl!("error-orchestrator-jira-status", status = *status),
            Self::Store(error) => fl!("error-orchestrator-jira-store", error = error.localize()),
            Self::Token => fl!("error-orchestrator-jira-token"),
            Self::TokenLength { limit } => fl!(
                "error-orchestrator-jira-token-length",
                limit = limit.to_string()
            ),
            Self::TooManySites => fl!("error-orchestrator-jira-too-many-sites", limit = MAX_SITES),
        }
    }
}
