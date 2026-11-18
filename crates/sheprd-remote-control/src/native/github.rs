//! GitHub for a phone that reads it itself (`docs/ios-github-plan.md`):
//! which repository each workspace is on ([`checkouts`]), which accounts
//! this computer lends a device, and `GET /api/v2/github/token`, the lease
//! of one.
//!
//! Sources, in order, per host: Sheprd's main account, for github.com only,
//! which Sheprd checks with GitHub before handing it over; then `gh auth
//! token --hostname HOST`. This daemon never renews a token: Sheprd's
//! refresh token stays with Sheprd, as GitHub voids the old pair on every
//! renewal. Additional accounts stay on the desktop.
//!
//! Only a device whose `share_github` switch is on gets a token, a few
//! times a minute at most, and a token is never logged: it lives in
//! [`Token`], whose `Debug` hides it.

mod asks;
pub(crate) mod checkouts;
mod error;
pub(crate) mod hosts;
pub(crate) mod remote;

pub(crate) use error::Error;

use crate::{
    auth::RateLimiter,
    control::{
        DeviceId,
        github::{GITHUB_COM, Lease, LeaseAnswer, MAX_HOST, Token, valid_host},
    },
    web::{ApiError, Device, Shared, State as Server, now_ms},
};
use axum::{
    Json,
    extract::{Query, State},
};
use hosts::{GhHost, Recognized};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};
use tokio::sync::{Semaphore, watch};

/// How long a phone's ask waits for Sheprd: its status poll, a take, and a
/// check with GitHub.
const LEASE_WAIT: Duration = Duration::from_secs(15);
/// Tokens one device may ask for at once, and how often it earns one back:
/// a handful a minute.
const LEASE_BURST: u32 = 5;
const LEASE_REFILL_MS: u64 = 12_000;
/// Devices the limit remembers.
const LEASE_DEVICES: usize = 256;
/// `gh` reads at once, across every phone.
const GH_READS: usize = 2;
/// How often `gh`'s hosts are read again.
const GH_HOSTS_EVERY: Duration = Duration::from_secs(60);

/// Reads `gh auth token --hostname HOST`. Blocking.
pub(crate) type GhToken = fn(&str) -> sheprd_github::Result<SecretString>;

/// What lending GitHub sign-ins keeps.
pub(crate) struct Lending {
    /// Phones' asks for Sheprd's sign-in.
    pub(crate) asks: asks::Asks,
    pub(crate) checkouts: checkouts::Checkouts,
    /// `[remote] github_hosts`.
    configured: Vec<String>,
    gh_hosts: watch::Sender<Arc<Vec<GhHost>>>,
    gh_reads: Semaphore,
    gh_token: GhToken,
    limit: Mutex<RateLimiter<DeviceId>>,
    /// Bumped when a device's switch changes, so homes are sent again.
    shares: watch::Sender<u64>,
    wait: Duration,
}

impl Default for Lending {
    fn default() -> Self {
        Self::new(Vec::new(), sheprd_github::cli::token_for)
    }
}

impl Lending {
    /// `configured` lists the user's own GitHub hosts; hosts that are not
    /// host names are dropped.
    pub(crate) fn new(configured: Vec<String>, gh_token: GhToken) -> Self {
        let configured = configured
            .into_iter()
            .map(|host| host.trim().to_ascii_lowercase())
            .filter(|host| valid_host(host))
            .collect();
        Self {
            asks: asks::Asks::default(),
            checkouts: checkouts::Checkouts::default(),
            configured,
            gh_hosts: watch::Sender::new(Arc::default()),
            gh_reads: Semaphore::new(GH_READS),
            gh_token,
            limit: Mutex::new(RateLimiter::new(
                LEASE_BURST,
                LEASE_REFILL_MS,
                LEASE_DEVICES,
            )),
            shares: watch::Sender::new(0),
            wait: LEASE_WAIT,
        }
    }

    /// The same, waiting `wait` for Sheprd.
    #[cfg(test)]
    pub(crate) fn waiting(mut self, wait: Duration) -> Self {
        self.wait = wait;
        self
    }

    #[cfg(test)]
    pub(crate) fn set_gh_hosts(&self, hosts: Vec<GhHost>) {
        self.gh_hosts.send_replace(Arc::new(hosts));
    }

    /// The hosts this computer counts as GitHub's now.
    pub(crate) fn recognized(&self) -> Recognized {
        Recognized {
            configured: self.configured.clone(),
            gh: self.gh_hosts.borrow().as_ref().clone(),
        }
    }

    /// A device's switch changed.
    pub(crate) fn shares_changed(&self) {
        self.shares.send_modify(|revision| *revision += 1);
    }

    /// Changes whenever what any device is lent may have.
    pub(crate) fn watch(&self) -> Watch {
        Watch {
            gh_hosts: self.gh_hosts.subscribe(),
            login: self.asks.watch_login(),
            shares: self.shares.subscribe(),
            checkouts: self.checkouts.watch(),
        }
    }

    fn allow(&self, device: &DeviceId) -> bool {
        self.limit
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .allow(device.clone(), now_ms())
    }
}

/// What a home follows of lending.
pub(crate) struct Watch {
    checkouts: watch::Receiver<Arc<std::collections::HashMap<String, checkouts::Checkout>>>,
    gh_hosts: watch::Receiver<Arc<Vec<GhHost>>>,
    login: watch::Receiver<Option<String>>,
    shares: watch::Receiver<u64>,
}

impl Watch {
    /// Returns once any changed; `false` when one can no longer.
    pub(crate) async fn changed(&mut self) -> bool {
        tokio::select! {
            changed = self.checkouts.changed() => changed.is_ok(),
            changed = self.gh_hosts.changed() => changed.is_ok(),
            changed = self.login.changed() => changed.is_ok(),
            changed = self.shares.changed() => changed.is_ok(),
        }
    }
}

/// Whether `device` is paired and may borrow this computer's sign-in.
fn shares(state: &Server, device: &DeviceId) -> bool {
    state
        .auth()
        .devices()
        .get(device)
        .is_some_and(|record| record.share_github)
}

/// One account this computer would lend a device.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct LentHost {
    pub(crate) host: String,
    pub(crate) login: String,
}

/// The accounts this computer would lend `device` now: none while its
/// switch is off.
pub(crate) fn lent_hosts(state: &Server, device: &DeviceId) -> Vec<LentHost> {
    if !shares(state, device) {
        return Vec::new();
    }
    let mut hosts = Vec::new();
    if state.changes.open()
        && let Some(login) = state.lending.asks.login()
    {
        hosts.push(LentHost {
            host: GITHUB_COM.to_owned(),
            login,
        });
    }
    for gh in state.lending.gh_hosts.borrow().iter() {
        if !hosts.iter().any(|lent| lent.host == gh.host) {
            hosts.push(LentHost {
                host: gh.host.clone(),
                login: gh.login.clone(),
            });
        }
    }
    hosts
}

/// Reads `gh`'s hosts now and every [`GH_HOSTS_EVERY`].
pub(crate) async fn follow_gh_hosts(state: Shared) {
    // Probing the login shell blocks, once; the path is kept after.
    let Ok(Some(path)) = tokio::task::spawn_blocking(hosts::file).await else {
        return;
    };
    let mut tick = tokio::time::interval(GH_HOSTS_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let path = path.clone();
        if let Ok(read) = tokio::task::spawn_blocking(move || hosts::read(&path)).await {
            state.lending.gh_hosts.send_if_modified(|known| {
                if known.as_slice() == read.as_slice() {
                    return false;
                }
                *known = Arc::new(read);
                true
            });
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct TokenQuery {
    host: String,
}

/// Where a lent token came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Source {
    Gh,
    Sheprd,
}

/// `GET /api/v2/github/token`'s answer.
#[derive(Debug, Serialize)]
pub(crate) struct Lent {
    host: String,
    login: String,
    token: Token,
    /// Seconds since the Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<u64>,
    source: Source,
}

/// `GET /api/v2/github/token?host=HOST`.
pub(crate) async fn token(
    State(state): State<Shared>,
    Device(device): Device,
    Query(query): Query<TokenQuery>,
) -> Result<Json<Lent>, ApiError> {
    Ok(Json(lend(&state, &device, &query.host).await?))
}

async fn lend(state: &Shared, device: &DeviceId, host: &str) -> Result<Lent, Error> {
    if !shares(state, device) {
        return Err(Error::NotShared);
    }
    if !state.lending.allow(device) {
        return Err(Error::RateLimited);
    }
    if host.len() > MAX_HOST {
        return Err(Error::InvalidHost);
    }
    let host = host.trim().to_ascii_lowercase();
    if !valid_host(&host) {
        return Err(Error::InvalidHost);
    }
    let mut timed_out = false;
    if host == GITHUB_COM && state.changes.open() && state.lending.asks.login().is_some() {
        match ask_sheprd(state, host.clone()).await {
            Ok(Some(lease)) => {
                return lent(state, device, host, lease, Source::Sheprd);
            }
            Ok(None) | Err(Error::Busy) => {}
            Err(_) => timed_out = true,
        }
    }
    let recognized = state.lending.recognized();
    if !recognized.is_github(&host) {
        return Err(Error::NoAccount);
    }
    match gh_token(state, &host).await {
        Ok(token) => {
            let login = recognized.gh_login(&host).unwrap_or_default().to_owned();
            let lease = Lease {
                login,
                token,
                expires_at: None,
            };
            lent(state, device, host, lease, Source::Gh)
        }
        Err(Error::Busy) => Err(Error::Busy),
        Err(_) if timed_out => Err(Error::TimedOut),
        Err(error) => Err(error),
    }
}

/// The lease for `device`, unless its switch went off or it was revoked
/// while the token was read.
fn lent(
    state: &Server,
    device: &DeviceId,
    host: String,
    lease: Lease,
    source: Source,
) -> Result<Lent, Error> {
    if !shares(state, device) {
        return Err(Error::NotShared);
    }
    tracing::info!(host, ?source, "Lent a GitHub sign-in to a phone");
    Ok(Lent {
        host,
        login: lease.login,
        token: lease.token,
        expires_at: lease.expires_at,
        source,
    })
}

/// Sheprd's current lease for `host`; `None` when it has none to give.
async fn ask_sheprd(state: &Server, host: String) -> Result<Option<Lease>, Error> {
    let mut pending = state.lending.asks.ask(host)?;
    match tokio::time::timeout(state.lending.wait, pending.answer()).await {
        Ok(Some(LeaseAnswer::Lease(lease))) => Ok(Some(lease)),
        Ok(Some(LeaseAnswer::SignedOut)) => Ok(None),
        Ok(None) | Err(_) => Err(Error::TimedOut),
    }
}

/// `gh`'s token for `host`, on the blocking pool, a few at a time.
async fn gh_token(state: &Server, host: &str) -> Result<Token, Error> {
    let _permit = state
        .lending
        .gh_reads
        .try_acquire()
        .map_err(|_| Error::Busy)?;
    let read = state.lending.gh_token;
    let host = host.to_owned();
    let token = tokio::task::spawn_blocking(move || read(&host))
        .await
        .map_err(|_| Error::NoAccount)?
        .map_err(|_| Error::NoAccount)?;
    let token = Token::new(token);
    if token.valid() {
        Ok(token)
    } else {
        Err(Error::NoAccount)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
