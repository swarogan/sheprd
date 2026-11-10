//! The open issues assigned to the user on a Jira Cloud site, read through
//! the site's `/rest/api/3`. Issues are keyed by their issue key, `PROJ-123`,
//! which is also the identifier shown.
//!
//! A site is one the user pasted an address, Atlassian account email, and API
//! token for in the account menu (`saved`). Nothing in a repository names a
//! Jira project: every repository has one source per saved site, keyed by the
//! site's hostname and the repository `assigned`, listing the issues
//! assigned to the token's owner. Nothing is ever written to Jira.

pub mod account;
pub mod adf;
mod error;
pub mod issue;
pub mod saved;

pub use error::Error;

/// The `[jira]` table.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Saves pasted API tokens in the private `0600` `jira-credentials`
    /// file when no desktop keyring is running. Same meaning as `[github]`'s.
    pub allow_plaintext_credentials: bool,
}

impl Config {
    /// Where pasted API tokens are saved.
    pub fn store(&self) -> sheprd_credentials::Store {
        sheprd_credentials::Store::for_policy(self.allow_plaintext_credentials)
    }
}

use super::{Checkpoint, Item, Provider, SourceKey};
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{collections::HashSet, io::Read, time::Duration};

/// The repository every Jira source is keyed by: the issues assigned to the
/// signed-in user, whichever project they are in.
pub const REPOSITORY: &str = "assigned";
/// What a source lists: the user's own unfinished issues, newest first.
pub const JQL: &str = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";
/// The fields a listing asks for, which are the ones it maps.
pub const FIELDS: [&str; 12] = [
    "summary",
    "description",
    "status",
    "priority",
    "issuetype",
    "labels",
    "assignee",
    "reporter",
    "parent",
    "created",
    "updated",
    "project",
];
/// Jira's largest page for this search.
const PAGE: usize = 100;
/// The most issues one sync reads; a longer list is shown as capped.
pub const MAX_OPEN: usize = 1_000;
const TIMEOUT: Duration = Duration::from_secs(30);
/// One page of 100 issues, descriptions included.
const LIMIT: u64 = 8 * 1024 * 1024;

/// The source of `host`'s issues assigned to the user.
pub fn source_key(host: &str) -> SourceKey {
    SourceKey {
        provider: Provider::Jira,
        host: host.to_owned(),
        repository: REPOSITORY.to_owned(),
    }
}

/// The sources of every saved site, sorted by hostname.
pub fn source_keys(saved: &saved::Saved) -> Vec<SourceKey> {
    saved.sites().map(|(host, _)| source_key(host)).collect()
}

/// `base_url` followed by `segments`, each escaped as one path segment.
pub(crate) fn endpoint(base_url: &str, segments: &[&str]) -> Result<url::Url, Error> {
    let invalid = || Error::Address(base_url.to_owned());
    let mut url = url::Url::parse(base_url).map_err(|_| invalid())?;
    url.path_segments_mut()
        .map_err(|()| invalid())?
        .pop_if_empty()
        .extend(segments);
    Ok(url)
}

/// The address of a REST call, `/rest/api/3/` and then `segments`.
pub(crate) fn api(base_url: &str, segments: &[&str]) -> Result<url::Url, Error> {
    let mut path = vec!["rest", "api", "3"];
    path.extend(segments);
    endpoint(base_url, &path)
}

/// One sync's result. Only a `complete` listing may replace the cache.
#[derive(Debug)]
pub struct Listing {
    pub items: Vec<Item>,
    pub checkpoint: Checkpoint,
    pub complete: bool,
    /// The token's owner, as Jira shows them; `None` when Jira did not say.
    pub viewer: Option<String>,
}

/// One request: a GET when `body` is `None`, else a POST of it as JSON.
pub type Fetch<'a> = dyn FnMut(&url::Url, Option<&Value>) -> super::Result<Vec<u8>> + 'a;

/// Lists the issues assigned to the token's owner on the site at `base_url`,
/// through `fetch`, which sends one request. `previous` keeps
/// agent-launcher's markers, which Sheprd never adds.
pub fn list(
    source: &SourceKey,
    base_url: &str,
    previous: Option<&Checkpoint>,
    fetch: &mut Fetch<'_>,
) -> super::Result<Listing> {
    let started = Utc::now();
    let search = api(base_url, &["search", "jql"])?;
    let mut issues: Vec<issue::Issue> = Vec::new();
    let mut seen = HashSet::new();
    let mut tokens = HashSet::new();
    let mut next: Option<String> = None;
    let complete = loop {
        let mut request = json!({ "jql": JQL, "maxResults": PAGE, "fields": FIELDS });
        if let (Some(token), Some(object)) = (&next, request.as_object_mut()) {
            object.insert("nextPageToken".to_owned(), json!(token));
        }
        let page: issue::Page = parse(&fetch(&search, Some(&request))?)?;
        let before = issues.len();
        issues.extend(
            page.issues
                .into_iter()
                .filter(|node| seen.insert(node.key().to_owned())),
        );
        let more = match page.next_page_token.filter(|_| page.is_last != Some(true)) {
            Some(token) => token,
            None => break true,
        };
        // A page that adds nothing, or hands back a token already used,
        // would be asked for forever.
        if issues.len() >= MAX_OPEN || issues.len() == before || !tokens.insert(more.clone()) {
            break false;
        }
        next = Some(more);
    };
    issues.truncate(MAX_OPEN);
    let items = issues
        .into_iter()
        .map(|node| node.into_item(source, base_url))
        .collect();
    let viewer = reader(base_url, fetch)?;
    let previous = previous.cloned().unwrap_or_default();
    Ok(Listing {
        items,
        checkpoint: Checkpoint {
            updated_at: Some(started),
            etag: None,
            last_full_at: Some(Utc::now()),
            pr_details: previous.pr_details,
            pr_cursor: previous.pr_cursor,
        },
        complete,
        viewer,
    })
}

/// Who the token reads as on the site at `base_url`. A reply in another
/// shape names no one rather than failing the listing.
fn reader(base_url: &str, fetch: &mut Fetch<'_>) -> super::Result<Option<String>> {
    let bytes = fetch(&api(base_url, &["myself"])?, None)?;
    Ok(serde_json::from_slice::<account::Myself>(&bytes)
        .ok()
        .and_then(account::Myself::name))
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(Error::Json)
}

/// Lists `source` with the sign-in of `site` over its own address. Blocks on
/// the network, which `cancelled` stops between requests: call it from a
/// worker.
pub fn sync(
    source: &SourceKey,
    site: Option<&saved::Site>,
    previous: Option<&Checkpoint>,
    cancelled: &impl Fn() -> bool,
    cooldown: &mut Option<Duration>,
) -> super::Result<Listing> {
    let Some(site) = site else {
        return Err(Error::SignIn {
            site: source.host.clone(),
        }
        .into());
    };
    let agent = agent(TIMEOUT);
    let target = Target {
        base_url: site.base_url(),
        email: site.email(),
        token: site.token(),
    };
    list(source, site.base_url(), previous, &mut |url, body| {
        if cancelled() {
            return Err(super::Error::Cancelled);
        }
        Ok(send(&agent, &target, url, body, LIMIT, cooldown)?)
    })
}

/// Whom a request is made as, and where.
pub(crate) struct Target<'a> {
    pub base_url: &'a str,
    pub email: &'a str,
    pub token: &'a secrecy::SecretString,
}

/// The `Authorization` header of `email` and `token`: Basic, over `email:token`.
pub(crate) fn authorization(
    email: &str,
    token: &secrecy::SecretString,
) -> Result<ureq::http::HeaderValue, Error> {
    let pair = zeroize::Zeroizing::new(format!(
        "{email}:{}",
        secrecy::ExposeSecret::expose_secret(token)
    ));
    let encoded =
        zeroize::Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(pair.as_bytes()));
    let mut value = ureq::http::HeaderValue::from_str(&format!("Basic {}", encoded.as_str()))
        .map_err(|_| Error::Token)?;
    value.set_sensitive(true);
    Ok(value)
}

/// Sends `body` as JSON to `url`, or a GET without one, to the site in
/// `target`, and returns the reply's body of at most `limit` bytes.
pub(crate) fn send(
    agent: &ureq::Agent,
    target: &Target<'_>,
    url: &url::Url,
    body: Option<&Value>,
    limit: u64,
    cooldown: &mut Option<Duration>,
) -> Result<Vec<u8>, Error> {
    let authorization = authorization(target.email, target.token)?;
    let network = |error| Error::Network {
        origin: target.base_url.to_owned(),
        source: Box::new(error),
    };
    let mut reply = match body {
        None => agent
            .get(url.as_str())
            .header("User-Agent", "Sheprd")
            .header("Accept", "application/json")
            .header("Authorization", authorization)
            .call()
            .map_err(network)?,
        Some(body) => agent
            .post(url.as_str())
            .header("User-Agent", "Sheprd")
            .header("Accept", "application/json")
            .header("Authorization", authorization)
            .header("Content-Type", "application/json")
            .send(&serde_json::to_vec(body).map_err(Error::Json)?[..])
            .map_err(network)?,
    };
    let status = reply.status().as_u16();
    if status != 200 {
        *cooldown = retry_after(status, reply.headers());
        tracing::warn!(
            category = "orchestrator_jira",
            status,
            "Jira refused a listing"
        );
        return Err(status_error(status, target.base_url));
    }
    read_body(&mut reply, limit)
}

/// An agent for Jira's API: bounded by `timeout`, following no redirect, so
/// a token is never carried to another address, and returning refusals as
/// statuses to read.
pub(crate) fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .into()
}

fn read_body(reply: &mut ureq::http::Response<ureq::Body>, limit: u64) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    reply
        .body_mut()
        .as_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(Error::Read)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Size);
    }
    Ok(bytes)
}

/// What a refusal means. Jira answers an email or token it does not accept
/// with 401, or 403 when the account may not use the API, and an address that
/// is not a Jira site with 404.
pub fn status_error(status: u16, origin: &str) -> Error {
    match status {
        401 | 403 => Error::Authentication,
        404 => Error::NotFound {
            origin: origin.to_owned(),
        },
        429 => Error::RateLimit,
        status => Error::Status(status),
    }
}

/// How long a rate-limited source waits: `retry-after` seconds, bounded to
/// between five minutes and a day.
pub fn retry_after(status: u16, headers: &ureq::http::HeaderMap) -> Option<Duration> {
    if status != 429 {
        return None;
    }
    let wait = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(3600);
    Some(Duration::from_secs(wait.clamp(300, 86400)))
}
