//! Listening on relays, so a paired phone reaches this computer from any
//! network (`docs/relay-plan.md`, Clients). For each relay in `[remote]
//! relays`, the daemon keeps one QUIC connection and listens on its route,
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

mod error;
pub(crate) mod keys;
mod listen;
mod serve;

pub use error::Error;
pub(crate) use error::Result;

pub(crate) use crate::control::RelayState as Status;
use serde::Serialize;
use sheprd_relay::Endpoint;
use sheprd_relay::wire::Key;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};
use tokio::sync::watch;

/// How long a pairing dial stays open on the relays: the nearby token's
/// claim window, with room for the phone to find a relay.
const PAIRING: Duration = Duration::from_secs(120);

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

/// The relays this computer listens on, and what it listens with.
#[derive(Debug)]
pub(crate) struct Relays {
    /// Bumped whenever a device's relay keys change, so each listener
    /// names them again.
    changed: watch::Sender<u64>,
    endpoints: Vec<Endpoint>,
    keys: keys::Keys,
    /// The open pairing dial, if any, and a revision listeners follow.
    pairing: Mutex<Option<Pairing>>,
    pairing_changed: watch::Sender<u64>,
    /// Each relay's state, for `sheprd remote status`.
    status: Mutex<BTreeMap<String, Status>>,
}

/// What a paired phone needs to reach this computer through its relays.
#[derive(Debug, Serialize)]
pub(crate) struct Info {
    /// The tunnel identity to pin: X25519 then ML-KEM-768, base64url.
    pub(crate) identity: String,
    /// The relays this computer listens on.
    pub(crate) relays: Vec<String>,
    /// This computer's route, base64url.
    pub(crate) route: String,
}

impl Relays {
    /// Loads or makes the relay keys in `state_dir`. Blocking.
    pub(crate) fn load(state_dir: &Path, endpoints: Vec<Endpoint>) -> Result<Self> {
        Ok(Self {
            changed: watch::Sender::new(0),
            endpoints,
            keys: keys::Keys::load_or_create(state_dir)?,
            pairing: Mutex::default(),
            pairing_changed: watch::Sender::new(0),
            status: Mutex::default(),
        })
    }

    /// What `GET /api/relay` answers.
    pub(crate) fn info(&self) -> Info {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        Info {
            identity: URL_SAFE_NO_PAD.encode(self.keys.tunnel.identity().to_bytes()),
            relays: self.endpoints.iter().map(ToString::to_string).collect(),
            route: self.keys.route().to_string(),
        }
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

    /// Each relay's state.
    pub(crate) fn status(&self) -> BTreeMap<String, Status> {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set_status(&self, endpoint: &Endpoint, status: Status) {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(endpoint.to_string(), status);
    }

    /// Starts a listener for each relay, serving `router` over each
    /// session, until `stopped`.
    pub(crate) fn start(self: &Arc<Self>, served: Served, stopped: watch::Receiver<bool>) {
        for endpoint in &self.endpoints {
            tokio::spawn(listen::run(
                Arc::clone(self),
                endpoint.clone(),
                served.clone(),
                stopped.clone(),
            ));
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
mod tests;
