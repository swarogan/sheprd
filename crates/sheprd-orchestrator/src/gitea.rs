//! A Gitea (or Forgejo) repository's open issues and pull requests, and its
//! latest settled pull requests, read through the instance's `/api/v1`. Issues
//! are keyed by their number and pull requests `pr/<number>`, both written
//! `#<number>` as Gitea does.
//!
//! An instance is one the user pasted an address and access token for in the
//! account menu (`saved`). A remote is a Gitea source when its hostname, port
//! aside, has one (`recognize`); the source keeps the remote's host and
//! repository, and requests go to the address that was typed. Listing never
//! writes; `write` changes one issue from its page.

pub mod account;
pub mod comments;
mod error;
pub mod issue;
pub mod pull_request;
pub mod saved;
pub mod write;

pub use error::Error;

/// The `[gitea]` table.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Saves pasted tokens in the private `0600` `gitea-credentials` file
    /// when no desktop keyring is running. Same meaning as `[github]`'s.
    pub allow_plaintext_credentials: bool,
}

impl Config {
    /// Where pasted tokens are saved.
    pub fn store(&self) -> sheprd_credentials::Store {
        sheprd_credentials::Store::for_policy(self.allow_plaintext_credentials)
    }
}

use super::{Checkpoint, Item, Provider, SourceKey};
use chrono::Utc;
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    io::Read,
    time::Duration,
};

/// Gitea's default and usual largest page.
const PAGE: usize = 50;
/// The most open issues, and as many open pull requests, one sync reads.
pub const MAX_OPEN: usize = 2_000;
/// How many of the latest closed or merged pull requests a sync keeps beside
/// the open ones, so the list can show what merged recently.
pub const RECENT_SETTLED: usize = 50;
const TIMEOUT: Duration = Duration::from_secs(30);
/// One page of 50 issues or pull requests, bodies included.
const LIMIT: u64 = 8 * 1024 * 1024;

/// Makes `source` a Gitea source when `saved` has an address for its host;
/// returns whether it changed. Only a guessed GitHub or GitLab source is
/// corrected, never `github.com`, and the host and repository stay as they
/// are. `saved` reads the credential store, only when needed.
pub fn recognize<S: Borrow<saved::Saved>>(
    source: &mut SourceKey,
    saved: impl FnOnce() -> S,
) -> bool {
    let guessed = matches!(source.provider, Provider::Github | Provider::Gitlab);
    if !guessed || saved::hostname(&source.host) == "github.com" {
        return false;
    }
    let known = saved().borrow().get(&source.host).is_some();
    if known {
        source.provider = Provider::Gitea;
    }
    known
}

/// `base_url` followed by `/api/v1/` and `segments`, each escaped as one
/// path segment.
pub(crate) fn endpoint(base_url: &str, segments: &[&str]) -> Result<url::Url, Error> {
    let invalid = || Error::Address(base_url.to_owned());
    let mut url = url::Url::parse(base_url).map_err(|_| invalid())?;
    url.path_segments_mut()
        .map_err(|()| invalid())?
        .pop_if_empty()
        .extend(["api", "v1"])
        .extend(segments);
    Ok(url)
}

/// A source's owner and repository, on the instance at `base_url`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repository<'a> {
    pub base_url: &'a str,
    pub owner: &'a str,
    pub name: &'a str,
}

impl<'a> Repository<'a> {
    /// `source`'s repository. A remote on an instance served from a
    /// sub-path names that path first, which is dropped.
    pub fn of(source: &'a SourceKey, base_url: &'a str) -> Result<Self, Error> {
        let prefix = url::Url::parse(base_url)
            .map(|url| url.path().trim_matches('/').to_owned())
            .unwrap_or_default();
        let path = source.repository.as_str();
        let path = path
            .strip_prefix(prefix.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .filter(|_| !prefix.is_empty())
            .unwrap_or(path);
        match path.split_once('/') {
            Some((owner, name)) if !owner.is_empty() && !name.is_empty() && !name.contains('/') => {
                Ok(Self {
                    base_url,
                    owner,
                    name,
                })
            }
            _ => Err(Error::Address(source.repository.clone())),
        }
    }

    fn api(&self, segments: &[&str]) -> Result<url::Url, Error> {
        let mut path = vec!["repos", self.owner, self.name];
        path.extend(segments);
        endpoint(self.base_url, &path)
    }

    /// The repository's web address, which item links hang from.
    pub fn web(&self) -> Result<String, Error> {
        let invalid = || Error::Address(self.base_url.to_owned());
        let mut url = url::Url::parse(self.base_url).map_err(|_| invalid())?;
        url.path_segments_mut()
            .map_err(|()| invalid())?
            .pop_if_empty()
            .push(self.owner)
            .push(self.name);
        Ok(url.as_str().to_owned())
    }

    /// Page `page` (from 1) of the open issues, pull requests left out.
    pub fn issues_url(&self, page: usize) -> Result<url::Url, Error> {
        let mut url = self.api(&["issues"])?;
        url.query_pairs_mut()
            .append_pair("state", "open")
            .append_pair("type", "issues")
            .append_pair("page", &page.to_string())
            .append_pair("limit", &PAGE.to_string());
        Ok(url)
    }

    /// Page `page` (from 1) of the open pull requests.
    pub fn pull_requests_url(&self, page: usize) -> Result<url::Url, Error> {
        let mut url = self.api(&["pulls"])?;
        url.query_pairs_mut()
            .append_pair("state", "open")
            .append_pair("page", &page.to_string())
            .append_pair("limit", &PAGE.to_string());
        Ok(url)
    }

    /// The latest [`RECENT_SETTLED`] closed or merged pull requests, most
    /// recently updated first.
    pub fn settled_url(&self) -> Result<url::Url, Error> {
        let mut url = self.api(&["pulls"])?;
        url.query_pairs_mut()
            .append_pair("state", "closed")
            .append_pair("sort", "recentupdate")
            .append_pair("page", "1")
            .append_pair("limit", &RECENT_SETTLED.to_string());
        Ok(url)
    }
}

/// One sync's result. Only a `complete` listing may replace the cache.
#[derive(Debug)]
pub struct Listing {
    pub items: Vec<Item>,
    pub checkpoint: Checkpoint,
    pub complete: bool,
    /// The token's owner, as authors are named, so the lists can keep the
    /// reader's own; `None` when Gitea did not say.
    pub viewer: Option<String>,
    /// The canonical keys of the pull requests the token's owner opened.
    pub authored: HashSet<String>,
}

/// Lists `source`'s open issues and pull requests, and its latest settled
/// pull requests, on the instance at `base_url`, through `fetch`, which
/// sends one request. `previous` keeps agent-launcher's markers, which
/// Sheprd never adds.
pub fn list(
    source: &SourceKey,
    base_url: &str,
    previous: Option<&Checkpoint>,
    fetch: &mut impl FnMut(url::Url) -> super::Result<Vec<u8>>,
) -> super::Result<Listing> {
    let started = Utc::now();
    let repository = Repository::of(source, base_url)?;
    let web = repository.web()?;
    let (issues, issues_complete) = pages(
        |page| repository.issues_url(page),
        issue::IssueNode::number,
        fetch,
    )?;
    let (pulls, pulls_complete) = pages(
        |page| repository.pull_requests_url(page),
        pull_request::PullRequestNode::number,
        fetch,
    )?;
    let settled: Vec<pull_request::PullRequestNode> = parse(&fetch(repository.settled_url()?)?)?;
    // A pull request settled while the open ones were read is listed once,
    // as the later read has it.
    let later: HashSet<u64> = settled.iter().map(|node| node.number()).collect();
    let mut authors = HashMap::new();
    let mut note = |node: &pull_request::PullRequestNode| {
        if let Some(login) = node.author() {
            authors.insert(node.number(), login.to_owned());
        }
    };
    let mut items: Vec<Item> = issues
        .into_iter()
        .filter(|node| !node.is_pull_request())
        .map(|node| node.into_item(source, &web))
        .collect();
    for node in pulls
        .into_iter()
        .filter(|node| !later.contains(&node.number()))
    {
        note(&node);
        items.push(node.into_item(source, &web));
    }
    for node in settled {
        note(&node);
        items.push(node.into_item(source, &web));
    }
    let viewer = reader(base_url, fetch)?;
    let authored = viewer.as_deref().map_or_else(HashSet::new, |me| {
        items
            .iter()
            .filter(|item| {
                item.pull_request
                    .as_ref()
                    .and_then(|pr| authors.get(&pr.number))
                    .is_some_and(|login| login.eq_ignore_ascii_case(me))
            })
            .map(|item| item.key.canonical())
            .collect()
    });
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
        complete: issues_complete && pulls_complete,
        viewer,
        authored,
    })
}

/// Who the token reads as on the instance at `base_url`. A reply in another
/// shape names no one rather than failing the listing.
fn reader(
    base_url: &str,
    fetch: &mut impl FnMut(url::Url) -> super::Result<Vec<u8>>,
) -> super::Result<Option<String>> {
    let bytes = fetch(endpoint(base_url, &["user"])?)?;
    Ok(serde_json::from_slice::<issue::User>(&bytes)
        .ok()
        .and_then(|user| user.login().map(str::to_owned)))
}

/// Every node of a listing, a page at a time, and whether all were read.
/// Pages are numbered, so one closed while they are read may shift another
/// out of view for this sync; the next sync lists it again. A page ends the
/// listing when it is short, and a full page that adds nothing new ends it
/// as incomplete rather than asking for the next one forever.
fn pages<T: DeserializeOwned>(
    url: impl Fn(usize) -> Result<url::Url, Error>,
    number: impl Fn(&T) -> u64,
    fetch: &mut impl FnMut(url::Url) -> super::Result<Vec<u8>>,
) -> super::Result<(Vec<T>, bool)> {
    let mut nodes = Vec::new();
    let mut seen = HashSet::new();
    let mut page = 1;
    loop {
        let batch: Vec<T> = parse(&fetch(url(page)?)?)?;
        let more = batch.len() >= PAGE;
        let before = nodes.len();
        nodes.extend(batch.into_iter().filter(|node| seen.insert(number(node))));
        if !more {
            return Ok((nodes, true));
        }
        if nodes.len() >= MAX_OPEN || nodes.len() == before {
            return Ok((nodes, false));
        }
        page += 1;
    }
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(Error::Json)
}

/// Lists `source` with the token of `instance` over its own address. Blocks
/// on the network, which `cancelled` stops between requests: call it from a
/// worker.
pub fn sync(
    source: &SourceKey,
    instance: Option<&saved::Instance>,
    previous: Option<&Checkpoint>,
    cancelled: &impl Fn() -> bool,
    cooldown: &mut Option<Duration>,
) -> super::Result<Listing> {
    let Some(instance) = instance else {
        return Err(Error::SignIn {
            repository: source.repository.clone(),
        }
        .into());
    };
    let agent = agent(TIMEOUT);
    list(source, instance.base_url(), previous, &mut |url| {
        if cancelled() {
            return Err(super::Error::Cancelled);
        }
        Ok(get(
            &agent,
            instance.base_url(),
            instance.token(),
            &url,
            &source.repository,
            LIMIT,
            cooldown,
        )?)
    })
}

/// Sends a GET for `url` to the instance at `base_url` with `token`;
/// `subject`, a repository or address, names what a refusal was for.
fn get(
    agent: &ureq::Agent,
    base_url: &str,
    token: &secrecy::SecretString,
    url: &url::Url,
    subject: &str,
    limit: u64,
    cooldown: &mut Option<Duration>,
) -> Result<Vec<u8>, Error> {
    let mut authorization = ureq::http::HeaderValue::from_str(&zeroize::Zeroizing::new(format!(
        "token {}",
        secrecy::ExposeSecret::expose_secret(token)
    )))
    .map_err(|_| Error::Token)?;
    authorization.set_sensitive(true);
    let mut reply = agent
        .get(url.as_str())
        .header("User-Agent", "Sheprd")
        .header("Accept", "application/json")
        .header("Authorization", authorization)
        .call()
        .map_err(|error| Error::Network {
            origin: base_url.to_owned(),
            source: Box::new(error),
        })?;
    let status = reply.status().as_u16();
    if status != 200 {
        *cooldown = retry_after(status, reply.headers());
        tracing::warn!(
            category = "orchestrator_gitea",
            status,
            "Gitea refused a listing"
        );
        return Err(status_error(status, subject));
    }
    body(&mut reply, limit)
}

/// An agent for Gitea's API: bounded by `timeout`, following no redirect, so
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

fn body(reply: &mut ureq::http::Response<ureq::Body>, limit: u64) -> Result<Vec<u8>, Error> {
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

/// What a refusal means. Gitea answers a token it does not accept with 401,
/// or 403 when the token lacks a scope, and a repository the token cannot
/// see with 404.
pub fn status_error(status: u16, repository: &str) -> Error {
    match status {
        401 | 403 => Error::Authentication,
        404 => Error::NotFound {
            repository: repository.to_owned(),
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
