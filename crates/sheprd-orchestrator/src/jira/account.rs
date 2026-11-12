//! Whom a Jira site takes an email and API token for: what the account menu
//! checks before it saves a pasted site.

use super::{
    Error, GATEWAY, TIMEOUT, Target, accept, agent, api, endpoint, gateway_base, read_body,
    request, saved, send,
};
use secrecy::SecretString;
use serde::Deserialize;

/// The answer is small; anything larger is refused.
const LIMIT: u64 = 1024 * 1024;
/// The site's cloud id is one short JSON object.
const TENANT_LIMIT: u64 = 4096;

/// The account a sign-in reads as on a site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// Jira's id for the account.
    pub account_id: String,
    /// The address to save the sign-in under, as `saved` keeps it.
    pub base_url: String,
    /// The site's cloud id when only Atlassian's gateway accepts the token,
    /// which is then a token with scopes.
    pub cloud_id: Option<String>,
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
    check_at(host, base_url, GATEWAY, email, token)
}

/// [`check`] of a site whose normalized address is known, with the gateway at
/// `gateway`; tests reach a local server through it, which `check` refuses as
/// it is not `https`. A site that answers 401 is asked again through the
/// gateway, which takes the tokens with scopes.
pub(crate) fn check_at(
    host: String,
    base_url: String,
    gateway: &str,
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
    let url = api(&base_url, &["myself"])?;
    let reply = request(&agent(TIMEOUT), &target, &url, None)?;
    let (bytes, cloud_id) = if reply.status().as_u16() == 401 {
        let id = tenant_id(&base_url).ok_or(Error::Authentication)?;
        (through_gateway(gateway, &id, &email, &token)?, Some(id))
    } else {
        (accept(reply, &target, LIMIT, &mut None)?, None)
    };
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
        cloud_id,
        email: me.email_address.filter(|email| !email.is_empty()),
        host,
        name,
    })
}

/// The cloud id of the site at `base_url`, which it gives to anyone. The
/// request carries no sign-in and follows no redirect; `None` for any answer
/// that is not a valid id.
fn tenant_id(base_url: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Tenant {
        #[serde(rename = "cloudId")]
        cloud_id: Option<String>,
    }
    let url = endpoint(base_url, &["_edge", "tenant_info"]).ok()?;
    let mut reply = agent(TIMEOUT)
        .get(url.as_str())
        .header("User-Agent", "Sheprd")
        .header("Accept", "application/json")
        .call()
        .ok()?;
    if reply.status().as_u16() != 200 {
        return None;
    }
    let bytes = read_body(&mut reply, TENANT_LIMIT).ok()?;
    let tenant: Tenant = serde_json::from_slice(&bytes).ok()?;
    saved::checked_cloud_id(&tenant.cloud_id?)
}

/// `myself` through the gateway at `gateway`. Only a failure to reach it is
/// reported as such; a refusal or an answer it should not give is the
/// site's own: the sign-in is not accepted.
fn through_gateway(
    gateway: &str,
    cloud_id: &str,
    email: &str,
    token: &SecretString,
) -> Result<Vec<u8>, Error> {
    let origin = gateway_base(gateway, cloud_id);
    let target = Target {
        base_url: gateway,
        email,
        token,
    };
    match send(
        &agent(TIMEOUT),
        &target,
        &api(&origin, &["myself"])?,
        None,
        LIMIT,
        &mut None,
    ) {
        Err(error @ Error::Network { .. }) => Err(error),
        Err(_) => Err(Error::Authentication),
        Ok(bytes) => Ok(bytes),
    }
}
