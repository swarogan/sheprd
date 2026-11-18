//! Sheprd's GitHub sign-in, lent to a paired phone (`docs/ios-github-plan.md`).
//! Sheprd alone renews its GitHub App tokens, since GitHub voids the old
//! pair on every renewal, so a phone gets a lease: the current access token,
//! never the refresh token. The daemon cannot call Sheprd, so a phone's ask
//! waits in the daemon until Sheprd takes it with `Request::GitHubLeaseAsks`
//! and answers with `Request::GitHubLease`, as `changes` asks do.
//!
//! A token crosses this socket only inside [`Token`], whose `Debug` never
//! shows it.

use super::changes::AskId;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

/// The one host Sheprd's own sign-in is for.
pub const GITHUB_COM: &str = "github.com";
/// The longest host name, as DNS allows.
pub const MAX_HOST: usize = 253;
/// The longest login sent, above GitHub's own limit of 39.
pub const MAX_LOGIN: usize = 100;
/// The longest token sent, as `sheprd-github` stores them.
pub const MAX_TOKEN: usize = 4096;

/// A phone's ask for the sign-in Sheprd holds for `host`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LeaseAsk {
    pub id: AskId,
    pub host: String,
}

/// An access token. Serialized as the plain string, as the wire needs it;
/// `Debug` says only that it is there.
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct Token(SecretString);

impl Token {
    pub fn new(token: SecretString) -> Self {
        Self(token)
    }

    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    /// Whether it is a token GitHub could have issued: printable ASCII
    /// within [`MAX_TOKEN`].
    pub fn valid(&self) -> bool {
        let token = self.expose();
        !token.is_empty() && token.len() <= MAX_TOKEN && token.bytes().all(|b| b.is_ascii_graphic())
    }
}

impl From<SecretString> for Token {
    fn from(token: SecretString) -> Self {
        Self(token)
    }
}

impl Serialize for Token {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token([REDACTED])")
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.expose() == other.expose()
    }
}

impl Eq for Token {}

/// Sheprd's main account's current access token, checked with GitHub just
/// before it was sent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    pub login: String,
    pub token: Token,
    /// When the token stops working, in seconds since the Unix epoch, when
    /// Sheprd knows; a phone asks again on a 401 either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

impl Lease {
    /// Whether it stays within this module's bounds.
    pub fn valid(&self) -> bool {
        valid_login(&self.login) && self.token.valid()
    }
}

/// Sheprd's answer to a [`LeaseAsk`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer", content = "body")]
pub enum LeaseAnswer {
    Lease(Lease),
    /// Sheprd has no account for the host, or GitHub no longer takes its
    /// token. The daemon tries `gh` instead.
    #[serde(other)]
    SignedOut,
}

/// Whether `login` is one GitHub could name: 1 to [`MAX_LOGIN`] letters,
/// digits, and hyphens (Enterprise logins may also carry `_`).
pub fn valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= MAX_LOGIN
        && login
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// Whether `host` is a lowercase DNS name with at least two labels, such as
/// `github.com` or `ghe.example.com`: no scheme, port, path, or user.
pub fn valid_host(host: &str) -> bool {
    host.len() <= MAX_HOST
        && host.contains('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}
