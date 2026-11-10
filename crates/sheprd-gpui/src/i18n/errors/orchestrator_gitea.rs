//! Gitea failures from `error-orchestrator-gitea.ftl`. Network, parser, and
//! I/O text stays a placeable; a path or store failure is localized on its
//! own.

use super::Localize;
use crate::i18n::fl;
use sheprd_orchestrator::gitea::{Error, saved::MAX_INSTANCES};

impl Localize for Error {
    fn localize(&self) -> String {
        match self {
            Self::Address(address) => fl!(
                "error-orchestrator-gitea-address",
                address = format!("{address:?}")
            ),
            Self::Authentication => fl!("error-orchestrator-gitea-authentication"),
            Self::Directory(error) => fl!(
                "error-orchestrator-gitea-directory",
                error = error.localize()
            ),
            Self::Json(error) => fl!("error-orchestrator-gitea-json", error = error.to_string()),
            Self::Network { origin, source } => fl!(
                "error-orchestrator-gitea-network",
                origin = origin.as_str(),
                error = source.to_string()
            ),
            #[cfg(target_os = "linux")]
            Self::NoKeyring(_) => fl!("error-orchestrator-gitea-no-keyring"),
            Self::NoStore(_) => fl!("error-orchestrator-gitea-no-store"),
            Self::NotFound { repository } => fl!(
                "error-orchestrator-gitea-not-found",
                repository = repository.as_str()
            ),
            Self::NotGitea { origin } => {
                fl!(
                    "error-orchestrator-gitea-not-gitea",
                    origin = origin.as_str()
                )
            }
            Self::RateLimit => fl!("error-orchestrator-gitea-rate-limit"),
            Self::Read(error) => fl!("error-orchestrator-gitea-read", error = error.to_string()),
            Self::Record => fl!("error-orchestrator-gitea-record"),
            Self::RecordSize => fl!("error-orchestrator-gitea-record-size"),
            Self::SignIn { repository } => fl!(
                "error-orchestrator-gitea-sign-in",
                repository = repository.as_str()
            ),
            Self::Size => fl!("error-orchestrator-gitea-size"),
            Self::Status(status) => fl!("error-orchestrator-gitea-status", status = *status),
            Self::Store(error) => fl!("error-orchestrator-gitea-store", error = error.localize()),
            Self::Token => fl!("error-orchestrator-gitea-token"),
            Self::TokenLength { limit } => fl!(
                "error-orchestrator-gitea-token-length",
                limit = limit.to_string()
            ),
            Self::TooManyInstances => fl!(
                "error-orchestrator-gitea-too-many-instances",
                limit = MAX_INSTANCES
            ),
        }
    }
}
