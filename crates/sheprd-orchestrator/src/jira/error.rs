//! Why a Jira site could not be read, or its address and token could not be saved.

/// Why a Jira listing, check, or saved site failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "{0:?} is not an https address of a Jira Cloud site, such as https://acme.atlassian.net"
    )]
    Address(String),
    #[error(
        "Jira rejected the email or API token; replace them from the account menu with an Atlassian API token. A token with scopes needs the read:jira-work and read:jira-user scopes"
    )]
    Authentication,
    #[error("The folder for saved Jira API tokens could not be found: {0}")]
    Directory(#[source] sheprd_client::local_path::PathError),
    #[error("The Atlassian account email must be an address such as you@example.com")]
    Email,
    #[error("An Atlassian account email is at most {limit} characters")]
    EmailLength { limit: usize },
    #[error("Jira's answer is not the expected JSON: {0}")]
    Json(#[source] serde_json::Error),
    #[error("Jira could not be reached at {origin}: {source}")]
    Network {
        /// The site's address, so a wrong one shows.
        origin: String,
        #[source]
        source: Box<ureq::Error>,
    },
    #[cfg(target_os = "linux")]
    #[error(
        "No desktop keyring is running to save a Jira API token in. Start GNOME Keyring, KWallet, or KeePassXC, or set [jira] allow_plaintext_credentials = true"
    )]
    NoKeyring(#[source] sheprd_credentials::Error),
    #[error(
        "No secure credential store is available to save a Jira API token in. Set [jira] allow_plaintext_credentials = true"
    )]
    NoStore(#[source] sheprd_credentials::Error),
    #[error("{origin} does not answer as a Jira Cloud site; check the address")]
    NotFound { origin: String },
    #[error("Jira rate limit reached; it is read again later")]
    RateLimit,
    #[error("Jira's answer could not be read: {0}")]
    Read(#[source] std::io::Error),
    #[error("The saved Jira sites are not in the expected form")]
    Record,
    #[error(
        "The saved Jira sites would be too large to read back; forget a site or use a shorter token"
    )]
    RecordSize,
    #[error("Connect Jira from the account menu to read the issues assigned to you on {site}")]
    SignIn { site: String },
    #[error("Jira's answer exceeded the size limit")]
    Size,
    #[error("Jira request failed (HTTP {0})")]
    Status(u16),
    #[error("The Jira API token could not be read or saved: {0}")]
    Store(#[source] sheprd_credentials::Error),
    #[error("The Jira API token is not one an HTTP header can carry")]
    Token,
    #[error("A Jira API token is at most {limit} characters; paste the token Atlassian showed")]
    TokenLength { limit: usize },
    #[error("Jira sites are saved for at most {} sites", super::saved::MAX_SITES)]
    TooManySites,
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
    /// try rather than asking Jira every few minutes.
    pub fn needs_user(&self) -> bool {
        matches!(
            self,
            Self::Authentication | Self::NotFound { .. } | Self::SignIn { .. }
        )
    }

    /// A missing or refused token, which one saved from the account menu
    /// fixes; the view offers that page rather than a failure.
    pub fn needs_sign_in(&self) -> bool {
        matches!(self, Self::SignIn { .. } | Self::Authentication)
    }
}
