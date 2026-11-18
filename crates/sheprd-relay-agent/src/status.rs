//! What `status --json` reports, for Sheprd to read over SSH when it
//! enrolls this machine: the route and identity to pin, how many desktops
//! may dial, and, while the agent runs, each relay's state.
//!
//! The running agent writes `status.json` in its state directory whenever
//! a relay's state changes and every [`HEARTBEAT`], and removes it when it
//! stops. A file older than [`STALE`] is from an agent that died.

use crate::{Error, Result, authorized::Authorized, files, keys::Keys};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const FILE: &str = "status.json";
/// How often the running agent rewrites its status.
pub const HEARTBEAT: Duration = Duration::from_secs(30);
/// A status older than this is from an agent that is no longer running.
pub const STALE: Duration = Duration::from_secs(90);
const MAX_BYTES: u64 = 64 * 1024;

/// One relay's state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RelayState {
    Connecting,
    Listening,
    Failed { error: String },
}

/// What the running agent writes.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Live {
    /// Each relay, by endpoint.
    pub relays: BTreeMap<String, RelayState>,
    /// When it was written, in seconds since the Unix epoch.
    pub updated: u64,
}

impl Live {
    /// Writes this status to `state_dir`, stamped now. Blocking.
    pub fn save(&mut self, state_dir: &Path, now: SystemTime) -> Result<()> {
        self.updated = seconds(now);
        let path = state_dir.join(FILE);
        let json = serde_json::to_vec_pretty(self).map_err(|source| Error::StatusFormat {
            path: path.clone(),
            source,
        })?;
        files::write_private(&path, &json)
    }

    /// The running agent's status in `state_dir`, unless none is running:
    /// no file, or one older than [`STALE`] at `now`. Blocking.
    pub fn load(state_dir: &Path, now: SystemTime) -> Result<Option<Self>> {
        let path = state_dir.join(FILE);
        let Some(bytes) = files::read_bounded(&path, MAX_BYTES)? else {
            return Ok(None);
        };
        let live: Self = serde_json::from_slice(&bytes)
            .map_err(|source| Error::StatusFormat { path, source })?;
        let age = seconds(now).saturating_sub(live.updated);
        Ok((age <= STALE.as_secs()).then_some(live))
    }
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// What `status` prints.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Report {
    /// How many desktops may dial.
    pub authorized: usize,
    /// The tunnel identity desktops pin, base64url.
    pub identity: String,
    /// Each relay's state while the agent runs; empty when it does not.
    pub relays: BTreeMap<String, RelayState>,
    /// The route this agent listens on, base64url.
    pub route: String,
    /// Whether an agent is running for this state directory.
    pub running: bool,
    /// The agent's version.
    pub version: String,
}

impl Report {
    /// The report for `state_dir` at `now`, making the keys the first time
    /// so a machine being enrolled has its route before the agent starts.
    /// Blocking.
    pub fn read(state_dir: &Path, now: SystemTime) -> Result<Self> {
        let keys = Keys::load_or_create(state_dir)?;
        let authorized = Authorized::load(state_dir)?;
        let live = Live::load(state_dir, now)?;
        Ok(Self {
            authorized: authorized.entries().len(),
            identity: keys.identity(),
            relays: live
                .as_ref()
                .map(|live| live.relays.clone())
                .unwrap_or_default(),
            route: keys.route().to_string(),
            running: live.is_some(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        })
    }

    /// The report as a person reads it.
    #[must_use]
    pub fn text(&self) -> String {
        let mut text = format!(
            "route: {}\nrunning: {}\nauthorized desktops: {}\n",
            self.route,
            if self.running { "yes" } else { "no" },
            self.authorized
        );
        for (relay, state) in &self.relays {
            let state = match state {
                RelayState::Connecting => "connecting".to_owned(),
                RelayState::Listening => "listening".to_owned(),
                RelayState::Failed { error } => format!("failed: {error}"),
            };
            text.push_str(&format!("relay {relay}: {state}\n"));
        }
        text
    }
}
