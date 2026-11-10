//! Whom a Jira site takes an email and API token for: what the account menu
//! checks before it saves a pasted site.

use super::{Error, TIMEOUT, Target, agent, api, saved, send};
use secrecy::SecretString;
use serde::Deserialize;

/// The answer is small; anything larger is refused.
const LIMIT: u64 = 1024 * 1024;

/// The account a sign-in reads as on a site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// Jira's id for the account.
    pub account_id: String,
    /// The address to save the sign-in under, as `saved` keeps it.
    pub base_url: String,
    /// The email Jira shows for the account, when its privacy settings allow.
    pub email: Option<String>,
    /// The hostname the sign-in is saved for, without a port.
    pub host: String,
    /// The name Jira shows the account by.
    pub name: String,
}

/// The reply of `GET /rest/api/3/myself`.
#[derive(Deserialize)]
pub(super) struct Myself {
    #[serde(rename = "accountId")]
    account_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "emailAddress")]
    email_address: Option<String>,
}

impl Myself {
    /// The name Jira shows, if it gave one.
    pub(super) fn name(self) -> Option<String> {
        self.display_name.filter(|name| !name.trim().is_empty())
    }
}

/// Whom the site at `address` takes `email` and `token` for. Blocks on the
/// network: call it from a worker.
pub fn check(address: &str, email: &str, token: &SecretString) -> Result<Account, Error> {
    let (host, base_url) =
        saved::normalize_address(address).ok_or_else(|| Error::Address(address.to_owned()))?;
    check_at(host, base_url, email, token)
}

/// [`check`] of a site whose normalized address is known; tests reach a local
/// server through it, which `check` refuses as it is not `https`.
pub(crate) fn check_at(
    host: String,
    base_url: String,
    email: &str,
    token: &SecretString,
) -> Result<Account, Error> {
    let email = saved::checked_email(email)?;
    let token = saved::checked_token(token)?;
    let target = Target {
        base_url: &base_url,
        email: &email,
        token: &token,
    };
    let bytes = send(
        &agent(TIMEOUT),
        &target,
        &api(&base_url, &["myself"])?,
        None,
        LIMIT,
        &mut None,
    )?;
    let me: Myself = serde_json::from_slice(&bytes).map_err(Error::Json)?;
    // An answer that names no account is not a Jira site's.
    let account_id = me
        .account_id
        .filter(|id| !id.is_empty())
        .ok_or(Error::Authentication)?;
    let name = me
        .display_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| email.clone());
    Ok(Account {
        account_id,
        base_url,
        email: me.email_address.filter(|email| !email.is_empty()),
        host,
        name,
    })
}
