//! A side's static keys, and the public identity the other side pins at
//! pairing.

use crate::{Error, Result};
use ml_kem::{
    B32, KeyExport, Seed,
    ml_kem_768::{Ciphertext, DecapsulationKey, EncapsulationKey},
};
use sha2::{Digest, Sha256};
use std::fmt;
use x25519_dalek::{PublicKey, StaticSecret};

/// An X25519 key's length, public or secret.
pub const X25519_LEN: usize = 32;
/// An ML-KEM-768 encapsulation (public) key's length.
pub const MLKEM_PUBLIC_LEN: usize = 1_184;
/// An ML-KEM-768 ciphertext's length.
pub const MLKEM_CIPHERTEXT_LEN: usize = 1_088;
/// An ML-KEM-768 decapsulation key's seed, as kept: `d ‖ z`, FIPS 203's
/// compact form, which CryptoKit's `seedRepresentation` also uses.
pub const MLKEM_SEED_LEN: usize = 64;
/// An identity's encoding: the X25519 key, then the ML-KEM-768 key.
pub const IDENTITY_LEN: usize = X25519_LEN + MLKEM_PUBLIC_LEN;
/// What an identity's fingerprint hashes ahead of its keys.
const FINGERPRINT_CONTEXT: &[u8] = b"sheprd-tunnel/identity/v1\0";

/// One side's static keys: X25519 for Noise, ML-KEM-768 for the hybrid.
pub struct Keys {
    mlkem: DecapsulationKey,
    mlkem_seed: [u8; MLKEM_SEED_LEN],
    x25519: StaticSecret,
}

impl fmt::Debug for Keys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Keys")
            .field("identity", &self.identity().fingerprint())
            .finish_non_exhaustive()
    }
}

impl Keys {
    /// Fresh keys.
    pub fn generate() -> Result<Self> {
        let mut x25519 = [0; X25519_LEN];
        let mut seed = [0; MLKEM_SEED_LEN];
        getrandom::fill(&mut x25519).map_err(Error::Random)?;
        getrandom::fill(&mut seed).map_err(Error::Random)?;
        Ok(Self::from_secrets(x25519, seed))
    }

    /// The keys these secrets make: an X25519 secret, and an ML-KEM-768
    /// seed. Every value is a valid key.
    #[must_use]
    pub fn from_secrets(x25519: [u8; X25519_LEN], mlkem_seed: [u8; MLKEM_SEED_LEN]) -> Self {
        Self {
            mlkem: DecapsulationKey::from_seed(Seed::from(mlkem_seed)),
            mlkem_seed,
            x25519: StaticSecret::from(x25519),
        }
    }

    /// The secrets to keep, for [`Keys::from_secrets`]. Keep them as a
    /// private key: whoever holds them is this side.
    #[must_use]
    pub fn secrets(&self) -> ([u8; X25519_LEN], [u8; MLKEM_SEED_LEN]) {
        (self.x25519.to_bytes(), self.mlkem_seed)
    }

    /// What the other side pins.
    #[must_use]
    pub fn identity(&self) -> Identity {
        let mut mlkem = [0; MLKEM_PUBLIC_LEN];
        mlkem.copy_from_slice(&self.mlkem.encapsulation_key().to_bytes());
        Identity {
            mlkem: Box::new(mlkem),
            x25519: PublicKey::from(&self.x25519).to_bytes(),
        }
    }

    pub(crate) fn x25519_secret(&self) -> [u8; X25519_LEN] {
        self.x25519.to_bytes()
    }

    /// The shared secret in `ciphertext`, which a dialer encapsulated to
    /// this side's ML-KEM key. A ciphertext that is not one gives a secret
    /// no dialer knows (FIPS 203's implicit rejection), so the handshake
    /// fails at its next step rather than here.
    pub(crate) fn decapsulate(&self, ciphertext: &[u8]) -> Result<[u8; 32]> {
        use ml_kem::Decapsulate as _;
        let ciphertext = Ciphertext::try_from(ciphertext).map_err(|_| Error::Malformed)?;
        Ok(self.mlkem.decapsulate(&ciphertext).into())
    }
}

/// A side's public keys, as the other side pins them at pairing.
#[derive(Clone, Eq, PartialEq)]
pub struct Identity {
    mlkem: Box<[u8; MLKEM_PUBLIC_LEN]>,
    x25519: [u8; X25519_LEN],
}

impl fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Identity")
            .field(&self.fingerprint())
            .finish()
    }
}

impl Identity {
    /// Reads [`Identity::to_bytes`]' encoding, checking the ML-KEM key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != IDENTITY_LEN {
            return Err(Error::Identity);
        }
        let (x25519, mlkem) = bytes.split_at(X25519_LEN);
        let mut key = [0; MLKEM_PUBLIC_LEN];
        key.copy_from_slice(mlkem);
        // Refuses a key FIPS 203's encapsulation key check would.
        encapsulation_key(&key)?;
        let mut x = [0; X25519_LEN];
        x.copy_from_slice(x25519);
        Ok(Self {
            mlkem: Box::new(key),
            x25519: x,
        })
    }

    /// The X25519 key, then the ML-KEM-768 key: [`IDENTITY_LEN`] bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        [&self.x25519[..], &self.mlkem[..]].concat()
    }

    /// The X25519 key, Noise's static key.
    #[must_use]
    pub fn x25519(&self) -> [u8; X25519_LEN] {
        self.x25519
    }

    /// The SHA-256 the QR carries, and every handshake's prologue binds.
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        Sha256::new()
            .chain_update(FINGERPRINT_CONTEXT)
            .chain_update(self.x25519)
            .chain_update(&self.mlkem[..])
            .finalize()
            .into()
    }

    /// A ciphertext and the shared secret in it, for this identity's
    /// ML-KEM key, with `randomness` as FIPS 203's `m`.
    pub(crate) fn encapsulate(
        &self,
        randomness: &[u8; 32],
    ) -> Result<([u8; MLKEM_CIPHERTEXT_LEN], [u8; 32])> {
        let key = encapsulation_key(&self.mlkem)?;
        let (ciphertext, shared) = key.encapsulate_deterministic(&B32::from(*randomness));
        let mut bytes = [0; MLKEM_CIPHERTEXT_LEN];
        bytes.copy_from_slice(&ciphertext);
        Ok((bytes, shared.into()))
    }
}

fn encapsulation_key(bytes: &[u8; MLKEM_PUBLIC_LEN]) -> Result<EncapsulationKey> {
    let key =
        ml_kem::kem::Key::<EncapsulationKey>::try_from(&bytes[..]).map_err(|_| Error::Identity)?;
    EncapsulationKey::new(&key).map_err(|_| Error::Identity)
}
