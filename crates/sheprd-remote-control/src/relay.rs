//! Listening on relays, so a paired phone reaches this computer from any
//! network (`docs/relay-plan.md`, Clients). For each relay in `[remote]
//! relays`, and each one Sheprd on another computer shared with this one
//! (`control::relays`), the daemon keeps one QUIC connection and listens
//! on its route,
//! naming the dial keys of its paired devices. Each dial runs the
//! end-to-end tunnel (`sheprd-tunnel`), admitting only a paired device's
//! tunnel key, then serves the same API over it, over the same TLS as on
//! the network with the certificate this computer presents for
//! `127.0.0.1`: the phone reaches it through a loopback proxy at
//! `https://127.0.0.1:PORT`, so its pinning, sign-in, and bearer tokens
//! work unchanged, and TLS adds a layer inside the tunnel.
//!
//! A relay sees only the tunnel's ciphertext, and a device that is not
//! paired hears `offline` from the relay or, past it, nothing at all.
//!
//! Relays share nothing, so a phone always dials the relays this computer
//! listens on, never its own: two machines set up with different relays
//! still meet. To use the nearest, the daemon measures each relay's round
//! trip and `GET /api/relay` lists them nearest first, with those times,
//! and the phone adds its own leg to each before choosing (`nearest`).

mod error;
pub(crate) mod keys;
mod listen;
mod nearest;
mod serve;
pub mod shared;

pub use error::Error;
pub(crate) use error::Result;

use crate::control;
pub(crate) use crate::control::RelayState as Status;
use crate::control::RelayStatus;
use crate::relay_label::RelayLabel;
use crate::server::Relay;
use sheprd_relay::Endpoint;
use sheprd_relay::wire::Key;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};
use tokio::{runtime::Handle, sync::watch};

/// How long a pairing dial stays open on the relays: the nearby token's
/// claim window, with room for the phone to find a relay.
const PAIRING: Duration = Duration::from_secs(120);
/// How often a running daemon reads the shared relays' file again.
const SHARED_RECHECK: Duration = Duration::from_secs(60);

/// A pairing dial the relays let through once: the key a nearby phone
/// derives from the token sealed to it, and that token, the tunnel's
/// pairing secret.
#[derive(Clone, Debug)]
struct Pairing {
    key: Key,
    secret: [u8; 32],
    until: Instant,
}

/// Lets the phone a nearby offer was just sealed to pair through the
/// relays from another network, when this computer listens on any.
pub(crate) fn open_pairing(state: &crate::web::State) {
    let Some(relay) = &state.relay else {
        return;
    };
    let secret = state.auth().take_relay_pairing();
    if let Some(secret) = secret {
        relay.offer_pairing(secret);
    }
}

/// Has nearby offers say what the relays measure now, nearest first, as
/// `GET /api/relay` does, rather than what they said at start, before any
/// listened: called before each offer is sealed.
pub(crate) fn refresh_offer(relays: Option<&Relays>, auth: &mut crate::auth::Auth) {
    if let Some(relays) = relays {
        auth.set_relay_offer(relays.offer());
    }
}

/// The relays this computer listens on, and what it listens with.
#[derive(Debug)]
pub(crate) struct Relays {
    /// Bumped whenever a device's relay keys change, so each listener
    /// names them again.
    changed: watch::Sender<u64>,
    /// The relays `[remote] relays` names, listened on first.
    configured: Vec<Endpoint>,
    keys: keys::Keys,
    /// What `[remote] relays` calls each relay, by endpoint; unlabelled
    /// relays are left out.
    labels: BTreeMap<String, RelayLabel>,
    /// The open pairing dial, if any, and a revision listeners follow.
    pairing: Mutex<Option<Pairing>>,
    pairing_changed: watch::Sender<u64>,
    /// Each listening relay's round-trip time, keyed by endpoint.
    rtt: Mutex<BTreeMap<String, Duration>>,
    running: Mutex<Running>,
    /// What other computers shared, and the file that keeps it.
    shared: Mutex<control::relays::Shared>,
    shared_file: PathBuf,
    /// Each relay's state, for `sheprd remote status`.
    status: Mutex<BTreeMap<String, Status>>,
}

/// The listeners running, once [`Relays::start`] said what they serve.
#[derive(Debug, Default)]
struct Running {
    started: Option<Started>,
    /// Each listening relay's own stop, by endpoint.
    listeners: BTreeMap<String, watch::Sender<bool>>,
}

#[derive(Debug)]
struct Started {
    runtime: Handle,
    served: Served,
    stopped: watch::Receiver<bool>,
}

/// What a paired phone needs to reach this computer through its relays.
pub(crate) use crate::control::host_relay::Info;

impl Relays {
    /// Loads or makes the relay keys in `state_dir`, and the relays other
    /// computers shared with it. Blocking.
    pub(crate) fn load(state_dir: &Path, relays: Vec<Relay>) -> Result<Self> {
        let shared_file = shared::file(state_dir);
        let labels = relays
            .iter()
            .filter(|relay| !relay.label.is_empty())
            .map(|relay| (relay.endpoint.to_string(), relay.label.clone()))
            .collect();
        Ok(Self {
            changed: watch::Sender::new(0),
            configured: relays.into_iter().map(|relay| relay.endpoint).collect(),
            keys: keys::Keys::load_or_create(state_dir)?,
            labels,
            pairing: Mutex::default(),
            pairing_changed: watch::Sender::new(0),
            rtt: Mutex::default(),
            running: Mutex::default(),
            shared: Mutex::new(shared::load(&shared_file)),
            shared_file,
            status: Mutex::default(),
        })
    }

    /// Every relay to listen on: the configured ones, then the shared ones
    /// they do not already name.
    pub(crate) fn endpoints(&self) -> Vec<Endpoint> {
        let mut all = self.configured.clone();
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        for endpoint in shared.endpoints() {
            if !all.contains(&endpoint) {
                all.push(endpoint);
            }
        }
        all
    }

    /// What nearby offers carry: [`Self::info`] without labels, as their
    /// sealed bytes cross Bluetooth and match shared vectors; the phone
    /// learns the labels from `GET /api/relay` once paired.
    pub(crate) fn offer(&self) -> Option<Info> {
        self.info().map(Info::without_labels)
    }

    /// What `GET /api/relay` answers, while this computer listens on any
    /// relay.
    pub(crate) fn info(&self) -> Option<Info> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let endpoints = self.endpoints();
        if endpoints.is_empty() {
            return None;
        }
        let rtt = self.rtt.lock().unwrap_or_else(PoisonError::into_inner);
        Some(Info {
            identity: URL_SAFE_NO_PAD.encode(self.keys.tunnel.identity().to_bytes()),
            labels: self.labels.clone(),
            relays: nearest::first(&endpoints, &rtt),
            route: self.keys.route().to_string(),
            rtt_ms: nearest::millis(&rtt),
        })
    }

    /// Takes the relays another computer shares, keeps them, and listens on
    /// them at once; returns every relay listened on now. Blocking: it
    /// writes the file.
    pub(crate) fn share(
        self: &Arc<Self>,
        offer: control::relays::Offer,
    ) -> std::result::Result<Vec<String>, shared::Error> {
        {
            // The file is what stands, not this copy: a command that could
            // not reach this daemon (a busy pipe looks the same as none)
            // wrote its offer there, and it is built on, never written over.
            let _file = shared::lock(&self.shared_file)?;
            let mut next = shared::load(&self.shared_file);
            if next.offer(offer)? {
                shared::save(&self.shared_file, &next)?;
            }
            *self.shared.lock().unwrap_or_else(PoisonError::into_inner) = next;
        }
        self.follow();
        Ok(self.endpoints().iter().map(ToString::to_string).collect())
    }

    /// Opens a pairing dial for the phone holding `secret`, on every relay.
    pub(crate) fn offer_pairing(&self, secret: [u8; 32]) {
        let key = sheprd_relay::client::public(&sheprd_relay::route::pairing_dial_key(&secret));
        *self.pairing.lock().unwrap_or_else(PoisonError::into_inner) = Some(Pairing {
            key,
            secret,
            until: Instant::now() + PAIRING,
        });
        self.pairing_changed.send_modify(|revision| *revision += 1);
    }

    /// The open pairing dial's key, and how many seconds it has left.
    fn pairing_key(&self) -> Option<(Key, u16)> {
        let pairing = self.pairing.lock().unwrap_or_else(PoisonError::into_inner);
        let pairing = pairing.as_ref()?;
        let left = pairing.until.checked_duration_since(Instant::now())?;
        Some((
            pairing.key,
            u16::try_from(left.as_secs()).unwrap_or(u16::MAX).max(1),
        ))
    }

    /// The pairing secret, when `key` is the open pairing dial's.
    fn pairing_secret(&self, key: &Key) -> Option<[u8; 32]> {
        let pairing = self.pairing.lock().unwrap_or_else(PoisonError::into_inner);
        pairing
            .as_ref()
            .filter(|pairing| pairing.key == *key && pairing.until > Instant::now())
            .map(|pairing| pairing.secret)
    }

    /// A device's relay keys changed: every listener names them again.
    pub(crate) fn devices_changed(&self) {
        self.changed.send_modify(|revision| *revision += 1);
    }

    /// Each relay's state, with the latest round trip of those listening,
    /// and its label.
    pub(crate) fn status(&self) -> BTreeMap<String, RelayStatus> {
        let rtt = nearest::millis(&self.rtt.lock().unwrap_or_else(PoisonError::into_inner));
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(endpoint, state)| {
                let mut state = state.clone();
                if let Status::Listening { rtt_ms } = &mut state {
                    *rtt_ms = rtt.get(endpoint).copied();
                }
                let status = RelayStatus {
                    label: self.labels.get(endpoint).cloned().unwrap_or_default(),
                    state,
                };
                (endpoint.clone(), status)
            })
            .collect()
    }

    /// Records `endpoint`'s round trip while it listens; `None` forgets it,
    /// so a relay that dropped this computer is not offered as nearest.
    fn set_rtt(&self, endpoint: &Endpoint, rtt: Option<Duration>) {
        let mut times = self.rtt.lock().unwrap_or_else(PoisonError::into_inner);
        match rtt {
            Some(rtt) => times.insert(endpoint.to_string(), rtt),
            None => times.remove(&endpoint.to_string()),
        };
    }

    /// Records a listener's state, unless it was stopped meanwhile: a
    /// relay no longer listened on is not shown.
    fn set_status(&self, endpoint: &Endpoint, status: Status) {
        let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        let key = endpoint.to_string();
        if !running.listeners.contains_key(&key) {
            return;
        }
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, status);
    }

    /// Starts a listener for each relay, serving `router` over each
    /// session, until `stopped`; relays shared later start as they come.
    ///
    /// The control socket answers by now, so this reads the shared relays
    /// again: an offer written to the file while it did not, after
    /// [`Self::load`] read it, is kept. Then it reads the file every
    /// [`SHARED_RECHECK`], for an offer a command wrote there while this
    /// daemon ran but did not answer.
    pub(crate) fn start(self: &Arc<Self>, served: Served, stopped: watch::Receiver<bool>) {
        self.reload();
        let runtime = Handle::current();
        let mut until = stopped.clone();
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .started = Some(Started {
            runtime: runtime.clone(),
            served,
            stopped,
        });
        self.follow();
        let this = Arc::clone(self);
        runtime.spawn(async move {
            let mut recheck = tokio::time::interval(SHARED_RECHECK);
            recheck.tick().await;
            loop {
                tokio::select! {
                    changed = until.changed() => {
                        if changed.is_err() || *until.borrow() {
                            break;
                        }
                    }
                    _ = recheck.tick() => {
                        let this = Arc::clone(&this);
                        let _ = tokio::task::spawn_blocking(move || {
                            if this.reload() {
                                this.follow();
                            }
                        })
                        .await;
                    }
                }
            }
            this.follow();
        });
    }

    /// Takes the shared relays as the file has them. Returns whether they
    /// changed. The file's lock is held until the copy in memory is
    /// replaced, as [`Self::share`] holds it, so an offer saved meanwhile is
    /// never undone by the older file read before it. Blocking.
    fn reload(&self) -> bool {
        let _file = match shared::lock(&self.shared_file) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(%error, "could not read the shared relays");
                return false;
            }
        };
        let saved = shared::load(&self.shared_file);
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if *shared == saved {
            return false;
        }
        *shared = saved;
        true
    }

    /// Brings the running listeners in line with [`Self::endpoints`]:
    /// starts the new ones and stops the ones left out, or every one once
    /// the daemon stops. Also tells nearby offers whether to name relays.
    ///
    /// The relays wanted are read under the lock that orders these updates,
    /// so an update that read an older list can never finish after a newer
    /// one and start a relay since withdrawn.
    fn follow(self: &Arc<Self>) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        let wanted = self.endpoints();
        let Some(started) = &running.started else {
            return;
        };
        let stopping = *started.stopped.borrow();
        let wanted: Vec<Endpoint> = if stopping { Vec::new() } else { wanted };
        let (runtime, served) = (started.runtime.clone(), started.served.clone());
        let mut status = self.status.lock().unwrap_or_else(PoisonError::into_inner);
        running.listeners.retain(|key, stop| {
            let keep = wanted.iter().any(|endpoint| endpoint.to_string() == *key);
            if !keep {
                let _ = stop.send(true);
                status.remove(key);
            }
            keep
        });
        for endpoint in wanted {
            let key = endpoint.to_string();
            if running.listeners.contains_key(&key) {
                continue;
            }
            let (stop, stopped) = watch::channel(false);
            running.listeners.insert(key.clone(), stop);
            status.insert(key, Status::Connecting);
            runtime.spawn(listen::run(
                Arc::clone(self),
                endpoint,
                served.clone(),
                stopped,
            ));
        }
        drop(status);
        drop(running);
        if !stopping {
            served.state.auth().set_relay_offer(self.info());
        }
    }
}

/// What each relay session serves: the daemon's state and router, behind
/// the TLS the phone pins.
#[derive(Clone)]
pub(crate) struct Served {
    pub(crate) router: axum::Router,
    pub(crate) state: crate::web::Shared,
    pub(crate) tls: Arc<rustls::ServerConfig>,
}

impl std::fmt::Debug for Served {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Served").finish_non_exhaustive()
    }
}

#[cfg(test)]
pub(crate) mod tests;
