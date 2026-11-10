//! Whom a Gitea instance takes a token for: what the account menu checks
//! before it saves a pasted address and token.

use super::{Error, TIMEOUT, agent, endpoint, get, saved};
use secrecy::SecretString;
use serde::Deserialize;

/// The answer is small; anything larger is refused.
const LIMIT: u64 = 1024 * 1024;

/// The account a token signs in as on an instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The address to save the token under, as `saved` keeps it.
    pub base_url: String,
    /// The hostname the token is saved for, without a port.
    pub host: String,
    /// The sign-in name, which authors are shown by.
    pub login: String,
    /// The full name Gitea shows, else the sign-in name.
    pub name: String,
}

/// The reply of `GET /api/v1/user`.
#[derive(Deserialize)]
struct User {
    login: Option<String>,
    username: Option<String>,
    full_name: Option<String>,
}

/// Whom the instance at `address` takes `token` for. Blocks on the network:
/// call it from a worker.
pub fn check(address: &str, token: &SecretString) -> Result<Account, Error> {
    let (host, base_url) =
        saved::normalize_address(address).ok_or_else(|| Error::Address(address.to_owned()))?;
    let token = saved::checked_token(token)?;
    let bytes = get(
        &agent(TIMEOUT),
        &base_url,
        &token,
        &endpoint(&base_url, &["user"])?,
        &base_url,
        LIMIT,
        &mut None,
    )
    .map_err(|error| match error {
        Error::NotFound { .. } => Error::NotGitea {
            origin: base_url.clone(),
        },
        error => error,
    })?;
    let user: User = serde_json::from_slice(&bytes).map_err(Error::Json)?;
    let login = user
        .login
        .or(user.username)
        .filter(|login| !login.is_empty())
        .ok_or(Error::Authentication)?;
    let name = user
        .full_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| login.clone());
    Ok(Account {
        base_url,
        host,
        login,
        name,
    })
}
