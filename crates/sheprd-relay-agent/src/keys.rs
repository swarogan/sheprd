//! The agent's keys, kept in `keys.bin` in its state directory, private to
//! this user: the P-256 key whose SHA-256 is its route on every relay, then
//! the tunnel keys (X25519, and ML-KEM-768's seed) desktops pin.

use crate::{Error, Result, files};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::SigningKey;
use sheprd_relay::RouteId;
use std::path::Path;

pub const FILE: &str = "keys.bin";
const ROUTE_LEN: usize = 32;
const X25519_LEN: usize = sheprd_tunnel::keys::X25519_LEN;
const SEED_LEN: usize = sheprd_tunnel::keys::MLKEM_SEED_LEN;
const LEN: usize = ROUTE_LEN + X25519_LEN + SEED_LEN;

/// The keys this agent listens and answers with.
pub struct Keys {
    /// Proves the route on each relay.
    pub route: SigningKey,
    /// Answers each desktop's end-to-end handshake.
    pub tunnel: sheprd_tunnel::Keys,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Keys")
            .field("tunnel", &self.tunnel)
            .finish_non_exhaustive()
    }
}

impl Keys {
    /// The keys in `state_dir`, made and saved the first time. The state
    /// directory's lock is held throughout, so callers racing on a fresh
    /// directory all end up with the one set of keys saved. Blocking.
    pub fn load_or_create(state_dir: &Path) -> Result<Self> {
        let _lock = files::Lock::acquire(state_dir)?;
        let path = state_dir.join(FILE);
        let stored = || Error::StoredKey { path: path.clone() };
        if let Some(bytes) = files::read_bounded(&path, LEN as u64)? {
            let bytes: [u8; LEN] = bytes.try_into().map_err(|_| stored())?;
            let (route, rest) = bytes.split_at(ROUTE_LEN);
            let (x25519, seed) = rest.split_at(X25519_LEN);
            let route = SigningKey::from_slice(route).map_err(|_| stored())?;
            let x25519 = x25519.try_into().map_err(|_| stored())?;
            let seed = seed.try_into().map_err(|_| stored())?;
            let tunnel = sheprd_tunnel::Keys::from_secrets(x25519, seed);
            return Ok(Self { route, tunnel });
        }
        let mut secret = [0; ROUTE_LEN];
        // A scalar out of range is drawn again; vanishingly rare.
        let route = loop {
            getrandom::fill(&mut secret).map_err(|_| stored())?;
            if let Ok(key) = SigningKey::from_slice(&secret) {
                break key;
            }
        };
        let tunnel = sheprd_tunnel::Keys::generate()?;
        let (x25519, seed) = tunnel.secrets();
        files::write_private(&path, &[&secret[..], &x25519, &seed].concat())?;
        Ok(Self { route, tunnel })
    }

    /// The route this agent listens on.
    #[must_use]
    pub fn route(&self) -> RouteId {
        sheprd_relay::client::route(&self.route)
    }

    /// The tunnel identity desktops pin: X25519 then ML-KEM-768, base64url.
    #[must_use]
    pub fn identity(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.tunnel.identity().to_bytes())
    }
}
