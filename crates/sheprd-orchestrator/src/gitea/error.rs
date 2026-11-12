//! Why a Gitea instance could not be read, or its address and token could not be saved.

/// Why a Gitea listing, check, or saved address failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0:?} is not an address of a Gitea instance, such as http://gitea.example.com")]
    Address(String),
    #[error(
        "Gitea rejected the token; replace it from the account menu with one that can read repositories and issues"
    )]
    Authentication,
    #[error("Write a comment before posting it")]
    Comment,
    #[error("A comment is at most {limit} bytes")]
    CommentLength { limit: usize },
    #[error("The folder for saved Gitea tokens could not be found: {0}")]
    Directory(#[source] sheprd_client::local_path::PathError),
    #[error(
        "Gitea refused the change; the token needs the write:issue scope, and your account needs permission on the repository"
    )]
    Forbidden,
    #[error("That is not an issue of a Gitea repository")]
    Issue,
    #[error("Gitea's answer is not the expected JSON: {0}")]
    Json(#[source] serde_json::Error),
    #[error("Gitea could not be reached at {origin}: {source}")]
    Network {
        /// The instance's address, so a wrong one shows.
        origin: String,
        #[source]
        source: Box<ureq::Error>,
    },
    #[cfg(target_os = "linux")]
    #[error(
        "No desktop keyring is running to save a Gitea token in. Start GNOME Keyring, KWallet, or KeePassXC, or set [gitea] allow_plaintext_credentials = true"
    )]
    NoKeyring(#[source] sheprd_credentials::Error),
    #[error(
        "No secure credential store is available to save a Gitea token in. Set [gitea] allow_plaintext_credentials = true"
    )]
    NoStore(#[source] sheprd_credentials::Error),
    #[error("Gitea has no repository {repository}, or the token cannot see it")]
    NotFound { repository: String },
    #[error("{origin} does not answer as a Gitea instance; check the address and any sub-path")]
    NotGitea { origin: String },
    #[error("Gitea rate limit reached; it is read again later")]
    RateLimit,
    #[error("Gitea's answer could not be read: {0}")]
    Read(#[source] std::io::Error),
    #[error("The saved Gitea tokens are not in the expected form")]
    Record,
    #[error(
        "The saved Gitea tokens would be too large to read back; forget an instance or use a shorter token"
    )]
    RecordSize,
    #[error("Connect Gitea from the account menu to read {repository}")]
    SignIn { repository: String },
    #[error("Gitea's answer exceeded the size limit")]
    Size,
    #[error("Gitea request failed (HTTP {0})")]
    Status(u16),
    #[error("The Gitea token could not be read or saved: {0}")]
    Store(#[source] sheprd_credentials::Error),
    #[error("The Gitea token is not one an HTTP header can carry")]
    Token,
    #[error("A Gitea access token is at most {limit} characters; paste the token Gitea showed")]
    TokenLength { limit: usize },
    #[error(
        "Gitea tokens are saved for at most {} instances",
        super::saved::MAX_INSTANCES
    )]
    TooManyInstances,
}

impl From<sheprd_credentials::Error> for Error {
    fn from(error: sheprd_credentials::Error) -> Self {
        match error {
            #[cfg(target_os = "linux")]
            sheprd_credentials::Error::SecretServiceUnavailable => Self::NoKeyring(error),
            sheprd_credentials::Error::Policy => Self::NoStore(error),
            error => Self::Store(error),
        }
    }
}

impl Error {
    /// A failure only the user can fix, which waits longer before the next
    /// try rather than asking Gitea every few minutes.
    pub fn needs_user(&self) -> bool {
        matches!(
            self,
            Self::Authentication | Self::NotFound { .. } | Self::SignIn { .. }
        )
    }

    /// A missing or refused token, which a token saved from the account
    /// menu fixes; the view offers that page rather than a failure.
    pub fn needs_sign_in(&self) -> bool {
        matches!(self, Self::SignIn { .. } | Self::Authentication)
    }
}
