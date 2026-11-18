//! This computer's keys for the machines it added over Bluetooth, made once
//! and kept in the remote control state directory's `desktop` folder,
//! private to this user: the P-256 dial key relays let through, the tunnel
//! keys (X25519, and ML-KEM-768's seed) each machine admits, and the SSH
//! key it logs in with, made by `ssh-keygen` so `ssh` reads it as is.

use super::{Error, Result, files};
use p256::ecdsa::SigningKey;
use sheprd_client::relay_host::{IDENTITY, directory};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

const FILE: &str = "keys.bin";
const DIAL_LEN: usize = 32;
const X25519_LEN: usize = sheprd_tunnel::keys::X25519_LEN;
const SEED_LEN: usize = sheprd_tunnel::keys::MLKEM_SEED_LEN;
const LEN: usize = DIAL_LEN + X25519_LEN + SEED_LEN;

/// The keys this computer dials relay machines with.
pub(crate) struct Keys {
    pub(crate) dial: SigningKey,
    pub(crate) tunnel: sheprd_tunnel::Keys,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Keys").finish_non_exhaustive()
    }
}

impl Keys {
    /// The keys under `state_dir`, made and saved the first time. Blocking.
    pub(crate) fn load_or_create(state_dir: &Path) -> Result<Self> {
        let path = directory(state_dir).join(FILE);
        if let Some(bytes) = files::read_bounded(&path, LEN as u64)? {
            let bytes: [u8; LEN] = bytes.try_into().map_err(|_| Error::RecordFormat)?;
            let (dial, rest) = bytes.split_at(DIAL_LEN);
            let (x25519, seed) = rest.split_at(X25519_LEN);
            let dial = SigningKey::from_slice(dial).map_err(|_| Error::RecordFormat)?;
            let x25519 = x25519.try_into().map_err(|_| Error::RecordFormat)?;
            let seed = seed.try_into().map_err(|_| Error::RecordFormat)?;
            return Ok(Self {
                dial,
                tunnel: sheprd_tunnel::Keys::from_secrets(x25519, seed),
            });
        }
        let mut secret = [0; DIAL_LEN];
        // A scalar out of range is drawn again; vanishingly rare.
        let dial = loop {
            getrandom::fill(&mut secret).map_err(|_| Error::RecordFormat)?;
            if let Ok(key) = SigningKey::from_slice(&secret) {
                break key;
            }
        };
        let tunnel = sheprd_tunnel::Keys::generate()?;
        let (x25519, seed) = tunnel.secrets();
        files::write_private(&path, &[&secret[..], &x25519, &seed].concat())?;
        Ok(Self { dial, tunnel })
    }
}

/// The SSH key this computer logs in with, `ssh-ed25519 BASE64`, made with
/// `ssh-keygen` the first time. Blocking.
pub(crate) fn ssh_public_key(state_dir: &Path) -> Result<String> {
    let private = directory(state_dir).join(IDENTITY);
    if !private.exists() {
        files::create_private_dir(&directory(state_dir))?;
        let status = sheprd_client::new_command("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "sheprd", "-f"])
            .arg(&private)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| Error::SshKeygen(Some(error)))?;
        if !status.success() {
            return Err(Error::SshKeygen(None));
        }
    }
    let public = PathBuf::from(format!("{}.pub", private.display()));
    let text = files::read_bounded(&public, 1024)?.ok_or(Error::SshKeygen(None))?;
    let text = String::from_utf8(text).map_err(|_| Error::SshKeygen(None))?;
    let mut fields = text.split_ascii_whitespace();
    match (fields.next(), fields.next()) {
        (Some(kind @ "ssh-ed25519"), Some(blob)) => Ok(format!("{kind} {blob}")),
        _ => Err(Error::SshKeygen(None)),
    }
}
