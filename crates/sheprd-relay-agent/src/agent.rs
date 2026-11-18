//! The running agent: a listener on every relay, the `authorized` file
//! followed as it changes, and `status.json` kept current.

use crate::{
    Result,
    authorized::Authorized,
    files::RunLock,
    keys::Keys,
    listen,
    status::{self, Live, RelayState},
};
use sheprd_relay::{Endpoint, RouteId};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::sync::watch;

/// How often the `authorized` file is read again, so a desktop added or
/// removed over SSH takes effect without a restart.
const POLL: Duration = Duration::from_secs(2);

/// What the agent runs with.
#[derive(Debug)]
pub struct Options {
    /// Where admitted sessions go: the SSH server.
    pub forward: SocketAddr,
    pub relays: Vec<Relay>,
    pub state_dir: PathBuf,
}

/// A relay to listen on, and its operator's listen token, if it asks for
/// one.
#[derive(Clone)]
pub struct Relay {
    pub endpoint: Endpoint,
    pub token: Vec<u8>,
}

impl std::fmt::Debug for Relay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Relay")
            .field("endpoint", &self.endpoint)
            .field("token", &(!self.token.is_empty()).then_some("…"))
            .finish()
    }
}

/// The agent's keys, its desktops, and each relay's state.
#[derive(Debug)]
pub struct Agent {
    /// Keeps a second agent off this state directory while this one lives.
    _run: RunLock,
    authorized: watch::Sender<Authorized>,
    forward: SocketAddr,
    pub(crate) keys: Keys,
    relays: Vec<Relay>,
    state_dir: PathBuf,
    status: watch::Sender<BTreeMap<String, RelayState>>,
}

impl Agent {
    /// Loads or makes the keys and reads the desktops; refuses when
    /// another agent runs on the same state directory. Blocking.
    pub fn load(options: Options) -> Result<Arc<Self>> {
        let Options {
            forward,
            relays,
            state_dir,
        } = options;
        let run = RunLock::try_acquire(&state_dir)?;
        let keys = Keys::load_or_create(&state_dir)?;
        let authorized = watch::Sender::new(Authorized::load(&state_dir)?);
        Ok(Arc::new(Self {
            _run: run,
            authorized,
            forward,
            keys,
            relays,
            state_dir,
            status: watch::Sender::new(BTreeMap::new()),
        }))
    }

    /// The route this agent listens on.
    #[must_use]
    pub fn route(&self) -> RouteId {
        self.keys.route()
    }

    /// Reads the `authorized` file again; listeners and live sessions
    /// follow when it changed. A file that does not parse leaves the
    /// desktops as they were. Blocking.
    pub fn reload(&self) -> Result<()> {
        let authorized = Authorized::load(&self.state_dir)?;
        self.authorized.send_if_modified(|current| {
            let changed = *current != authorized;
            *current = authorized;
            changed
        });
        Ok(())
    }

    /// Each relay's state, by endpoint.
    #[must_use]
    pub fn relay_states(&self) -> BTreeMap<String, RelayState> {
        self.status.borrow().clone()
    }

    pub(crate) fn set_state(&self, endpoint: &Endpoint, state: RelayState) {
        self.status.send_modify(|status| {
            status.insert(endpoint.to_string(), state);
        });
    }

    pub(crate) fn authorized(&self) -> watch::Receiver<Authorized> {
        self.authorized.subscribe()
    }

    pub(crate) fn forward(&self) -> SocketAddr {
        self.forward
    }

    /// Listens on every relay until `stopped`, then removes `status.json`.
    pub async fn run(self: Arc<Self>, mut stopped: watch::Receiver<bool>) {
        for relay in &self.relays {
            tokio::spawn(listen::run(
                Arc::clone(&self),
                relay.clone(),
                stopped.clone(),
            ));
        }
        let mut status = self.status.subscribe();
        let mut poll = tokio::time::interval(POLL);
        let mut heartbeat = tokio::time::interval(status::HEARTBEAT);
        loop {
            tokio::select! {
                _ = poll.tick() => {
                    let agent = Arc::clone(&self);
                    let reloaded = tokio::task::spawn_blocking(move || agent.reload()).await;
                    if let Ok(Err(error)) = reloaded {
                        tracing::warn!(error = %error, "could not read the authorized desktops");
                    }
                }
                _ = heartbeat.tick() => self.save_status().await,
                _ = status.changed() => self.save_status().await,
                () = until_stopped(&mut stopped) => break,
            }
        }
        let path = self.state_dir.join(status::FILE);
        let _ = tokio::task::spawn_blocking(move || crate::files::remove(&path)).await;
    }

    async fn save_status(self: &Arc<Self>) {
        let agent = Arc::clone(self);
        let saved = tokio::task::spawn_blocking(move || {
            let mut live = Live {
                relays: agent.relay_states(),
                updated: 0,
            };
            live.save(&agent.state_dir, SystemTime::now())
        })
        .await;
        if let Ok(Err(error)) = saved {
            tracing::warn!(error = %error, "could not write the agent's status");
        }
    }
}

/// Resolves once `stopped` is set or its sender is gone. Holds no guard
/// across the caller's other branches, which keeps their futures `Send`.
async fn until_stopped(stopped: &mut watch::Receiver<bool>) {
    let _ = stopped.wait_for(|stop| *stop).await;
}
