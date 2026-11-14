//! This computer's relay keys, kept in `STATE_DIR/relay/`, private to this
//! user: the P-256 key whose SHA-256 is its route on every relay, and the
//! tunnel keys (X25519 and ML-KEM-768) phones pin.

use super::{Error, Result};
use crate::auth::devices::write_private;
use p256::ecdsa::SigningKey;
use std::{fs, io, path::Path};

const DIR: &str = "relay";
const ROUTE_KEY: &str = "route-key.bin";
const TUNNEL_KEYS: &str = "tunnel-keys.bin";

/// The keys this computer listens and answers with.
pub(crate) struct Keys {
    /// Proves the route on each relay.
    pub(crate) route: SigningKey,
    /// Answers each phone's end-to-end handshake.
    pub(crate) tunnel: sheprd_tunnel::Keys,
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
    /// The keys in `state_dir`, made and saved the first time. Blocking.
    pub(crate) fn load_or_create(state_dir: &Path) -> Result<Self> {
        let dir = state_dir.join(DIR);
        let route = match read(&dir.join(ROUTE_KEY))? {
            Some(bytes) => SigningKey::from_slice(&bytes).map_err(|_| Error::StoredKey)?,
            None => {
                let mut bytes = [0; 32];
                // A scalar out of range is drawn again; vanishingly rare.
                let key = loop {
                    getrandom::fill(&mut bytes).map_err(|_| Error::StoredKey)?;
                    if let Ok(key) = SigningKey::from_slice(&bytes) {
                        break key;
                    }
                };
                write_private(&dir.join(ROUTE_KEY), &bytes)?;
                key
            }
        };
        let tunnel = match read(&dir.join(TUNNEL_KEYS))? {
            Some(bytes) if bytes.len() == 32 + 64 => {
                let mut x25519 = [0; 32];
                let mut seed = [0; 64];
                x25519.copy_from_slice(&bytes[..32]);
                seed.copy_from_slice(&bytes[32..]);
                sheprd_tunnel::Keys::from_secrets(x25519, seed)
            }
            Some(_) => return Err(Error::StoredKey),
            None => {
                let keys = sheprd_tunnel::Keys::generate()?;
                let (x25519, seed) = keys.secrets();
                write_private(&dir.join(TUNNEL_KEYS), &[&x25519[..], &seed].concat())?;
                keys
            }
        };
        Ok(Self { route, tunnel })
    }

    /// The route this computer listens on.
    pub(crate) fn route(&self) -> sheprd_relay::RouteId {
        sheprd_relay::client::route(&self.route)
    }
}

fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::StoredKey),
    }
}
