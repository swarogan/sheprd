//! The phone's side of nearby pairing (`docs/ios-app-plan.md`, "Nearby
//! pairing, exactly"), run by a desktop adding another computer: the same
//! frames an iPhone writes over Bluetooth, checked with the daemon's own
//! commitment, code, and sealing functions, so both halves stay one
//! protocol.

use super::{Error, Result};
use crate::auth::{
    self, PublicKey, decode_fixed, encode,
    nearby::{self, code, commit, fingerprint_bytes, shared_secret, transcript},
};
use crate::control::{NearbyRevealed, NearbyStarted, host_relay::Info};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::{SecretKey, elliptic_curve::zeroize::Zeroizing};
use serde::Deserialize;
use serde_json::json;

/// Between the first and second frames: this side's key and nonce.
pub(crate) struct Exchange {
    secret: SecretKey,
    nonce: [u8; 32],
    started: Option<NearbyStarted>,
}

/// What the other computer sealed to this one, opened.
#[derive(Debug)]
pub(crate) struct Opened {
    /// The six digits both screens show.
    pub(crate) code: String,
    /// What the other computer calls itself.
    pub(crate) computer: String,
    /// Its certificate authority's SHA-256, which its TLS must present.
    pub(crate) fingerprint: [u8; 32],
    /// The claim token, also the relays' pairing secret.
    pub(crate) token: Zeroizing<[u8; 32]>,
    /// How to reach it through its relays, when it listens on any.
    pub(crate) relay: Option<Info>,
}

#[derive(Deserialize)]
struct Sealed {
    token: String,
    #[serde(default)]
    relay: Option<Info>,
}

/// An answer that is an error rather than the step's result.
#[derive(Deserialize)]
struct Refusal {
    error: String,
}

impl Exchange {
    /// A fresh key and nonce, and the first frame: `start`.
    pub(crate) fn start() -> Result<(Self, Vec<u8>)> {
        let secret = loop {
            if let Ok(secret) = SecretKey::from_slice(&auth::random_bytes::<32>()?) {
                break secret;
            }
        };
        let nonce = auth::random_bytes::<32>()?;
        let frame = json!({
            "type": "start",
            "public_key": encode(&secret.public_key().to_sec1_bytes()),
        });
        let exchange = Self {
            secret,
            nonce,
            started: None,
        };
        Ok((exchange, frame.to_string().into_bytes()))
    }

    /// Reads the other computer's commitment; the second frame: `reveal`,
    /// this side's nonce.
    pub(crate) fn reveal(&mut self, answer: &[u8]) -> Result<Vec<u8>> {
        let started: NearbyStarted = parse(answer)?;
        let frame = json!({
            "type": "reveal",
            "session": started.session,
            "nonce": encode(&self.nonce),
        });
        self.started = Some(started);
        Ok(frame.to_string().into_bytes())
    }

    /// Checks the other computer's nonce against its commitment, and opens
    /// what it sealed with the key both sides agreed on.
    pub(crate) fn open(self, answer: &[u8]) -> Result<Opened> {
        let started = self.started.ok_or(Error::NearbyFormat)?;
        let revealed: NearbyRevealed = parse(answer)?;
        let theirs = PublicKey::parse(&started.public_key)?;
        let mine = self.secret.public_key().to_sec1_bytes();
        let their_nonce: [u8; 32] =
            decode_fixed(&revealed.nonce).map_err(|_| auth::Error::NearbyCrypto)?;
        let committed: [u8; 32] =
            decode_fixed(&started.commit).map_err(|_| auth::Error::NearbyCrypto)?;
        // The other side committed to its nonce before it saw this one's,
        // so a man in the middle gets one guess at the code.
        let expected = commit(theirs.as_bytes(), &mine, &their_nonce);
        if !auth::constant_time_eq(&expected, &committed) {
            return Err(auth::Error::NearbyCrypto.into());
        }
        let fingerprint =
            fingerprint_bytes(&started.fingerprint).ok_or(auth::Error::NearbyCrypto)?;
        let transcript = transcript(
            &mine,
            theirs.as_bytes(),
            &self.nonce,
            &their_nonce,
            &fingerprint,
        );
        let shared = shared_secret(&self.secret, &theirs)?;
        let key = nearby::key(shared.as_slice(), &transcript)?;
        let sealed = URL_SAFE_NO_PAD
            .decode(&revealed.sealed)
            .map_err(|_| auth::Error::NearbyCrypto)?;
        let plaintext = nearby::open(&key, &sealed)?;
        let sealed: Sealed =
            serde_json::from_slice(&plaintext).map_err(|_| auth::Error::NearbyCrypto)?;
        let token: [u8; 32] = decode_fixed(&sealed.token).map_err(|_| auth::Error::NearbyCrypto)?;
        Ok(Opened {
            code: code(&transcript),
            computer: started.computer,
            fingerprint,
            token: Zeroizing::new(token),
            relay: sealed.relay.filter(Info::valid),
        })
    }
}

/// A step's answer, or the error the other computer gave instead.
fn parse<T: serde::de::DeserializeOwned>(answer: &[u8]) -> Result<T> {
    if let Ok(refusal) = serde_json::from_slice::<Refusal>(answer) {
        return Err(Error::NearbyRefused(
            refusal.error.chars().take(200).collect(),
        ));
    }
    serde_json::from_slice(answer).map_err(|_| Error::NearbyFormat)
}
