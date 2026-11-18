//! Carrying a phone's relay keys to a computer it cannot reach directly,
//! through another computer it can (`docs/relay-plan.md`, Clients).
//!
//! A phone learns a computer's relays, and hands over its own keys for
//! them, only over a path it already trusts (`GET`/`POST /api/relay`). A
//! computer that turned relays on while the phone was away is out of reach
//! until the phone meets it on its network again. When the phone reaches
//! another of its computers, that computer's Sheprd carries the keys
//! instead: the phone signs a [`Request`] with the device key it holds for
//! the far computer and posts it to the near one's daemon; Sheprd there
//! runs `herdr-remote-control relay-enroll` over SSH on the hosts it
//! connects to, and the one whose fingerprint the request names checks the
//! signature against the phone's own key, keeps the relay keys, and answers
//! with its relays ([`Outcome`]).
//!
//! The signature is what lets this reach a phone paired with the far
//! computer directly: the near computer only carries the request, and
//! cannot attach keys to a device it does not hold.
//!
//! The far computer signs its answer in turn ([`Proof`]), with the key of
//! the certificate authority whose fingerprint the phone pinned at pairing,
//! over the request and what it answers. Every host Sheprd asks could
//! otherwise claim to be that computer and hand the phone relays that lead
//! nowhere; an answer that does not check out is passed over, and the phone
//! checks it again before keeping it.

use super::{DeviceId, hosts::HostId};
use crate::relay_label::RelayLabel;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// What a phone signs, before the fields of [`Request::message`].
pub const PREFIX: &[u8] = b"sheprd-host-relay-v1";
/// What the far computer signs, before the request's message and its
/// answer ([`Outcome::message`]).
pub const ANSWER_PREFIX: &[u8] = b"sheprd-host-relay-answer-v1";
/// The largest certificate an answer may carry, base64url.
pub const MAX_CERTIFICATE: usize = 8192;
/// How old a signed request may be when it arrives: a near computer keeps a
/// request a day for Sheprd to take, and a replay inside it can only put
/// back keys the phone itself chose.
pub const MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;
/// How far ahead of the far computer's clock a request may be signed.
pub const MAX_SKEW_MS: u64 = 10 * 60 * 1000;
/// How long Sheprd tries hosts for one request it took, over SSH one
/// after another; the near daemon waits a little longer for its answer
/// (`auth::host_relays`), and the phone longer still.
pub const CARRY_MS: u64 = 8 * 60 * 1000;
/// The most relays one computer may name.
pub const MAX_RELAYS: usize = 16;
/// The longest field accepted: a relay URL, or the tunnel identity
/// (X25519 and ML-KEM-768, 1,216 bytes in base64url).
pub const MAX_FIELD: usize = 2048;
/// The longest failure reason kept for a phone to show.
pub const MAX_REASON: usize = 512;

/// A phone's keys for a computer's relays, base64url: its dial key (P-256,
/// SEC1 compressed), which the relays let through, and its tunnel static
/// key (X25519), which the end-to-end handshake proves.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Keys {
    pub dial: String,
    pub tunnel: String,
}

/// What a phone needs to reach a computer through its relays: the same
/// JSON as `GET /api/relay`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Info {
    /// The tunnel identity to pin: X25519 then ML-KEM-768, base64url.
    pub identity: String,
    /// What the user calls each relay, by endpoint as `relays` writes it;
    /// unlabelled relays are left out, and the field with them. A carried
    /// answer leaves them all out: the phone reads them from
    /// `GET /api/relay` once it reaches the computer, through the relay too.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, RelayLabel>,
    /// The relays the computer listens on, nearest first; those not
    /// listening now keep their configured order, after the rest.
    pub relays: Vec<String>,
    /// The computer's route, base64url.
    pub route: String,
    /// Each listening relay's round trip from the computer, in
    /// milliseconds, so the phone can add its own leg. Older phones ignore
    /// it. A carried answer does not sign it: it only orders relays the
    /// signature names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rtt_ms: BTreeMap<String, u32>,
}

impl Info {
    pub fn valid(&self) -> bool {
        field(&self.identity)
            && field(&self.route)
            && !self.relays.is_empty()
            && self.relays.len() <= MAX_RELAYS
            && self.relays.iter().all(|relay| field(relay))
            && self.rtt_ms.len() <= MAX_RELAYS
            && self.rtt_ms.keys().all(|relay| field(relay))
            && self.labels.len() <= MAX_RELAYS
            && self
                .labels
                .iter()
                .all(|(relay, label)| field(relay) && label.check().is_ok())
    }

    /// The info without labels, as nearby offers and carried answers send
    /// it.
    #[must_use]
    pub fn without_labels(self) -> Self {
        Self {
            labels: BTreeMap::new(),
            ..self
        }
    }
}

/// A phone's relay keys for one computer, signed with the device key it
/// holds for that computer.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The far computer's CA fingerprint, hex with or without colons.
    pub fingerprint: String,
    /// The phone's device on the far computer.
    pub device: DeviceId,
    pub keys: Keys,
    /// When the phone signed, in milliseconds since 1970.
    pub signed_ms: u64,
    /// ECDSA P-256 SHA-256 over [`Request::message`], `r || s`, base64url.
    pub signature: String,
}

impl Request {
    pub fn valid(&self) -> bool {
        self.signable() && field(&self.signature)
    }

    /// Whether the signed fields are in bounds, signature aside.
    fn signable(&self) -> bool {
        fingerprint_bytes(&self.fingerprint).is_some()
            && field(&self.device.0)
            && field(&self.keys.dial)
            && field(&self.keys.tunnel)
    }

    /// What the phone signs: [`PREFIX`], the fingerprint's 32 bytes, the
    /// device id, the dial key, and the tunnel key as their wire text, each
    /// after its length as a big-endian `u16`, then `signed_ms` as a
    /// big-endian `u64`. `None` when those fields are out of bounds; the
    /// signature is not part of it.
    pub fn message(&self) -> Option<Vec<u8>> {
        if !self.signable() {
            return None;
        }
        let fingerprint = fingerprint_bytes(&self.fingerprint)?;
        let mut message = Vec::with_capacity(256);
        message.extend_from_slice(PREFIX);
        message.extend_from_slice(&fingerprint);
        for text in [&self.device.0, &self.keys.dial, &self.keys.tunnel] {
            let length = u16::try_from(text.len()).ok()?;
            message.extend_from_slice(&length.to_be_bytes());
            message.extend_from_slice(text.as_bytes());
        }
        message.extend_from_slice(&self.signed_ms.to_be_bytes());
        Some(message)
    }

    /// Whether the request was signed recently enough for a computer whose
    /// clock reads `now_ms`.
    pub fn fresh(&self, now_ms: u64) -> bool {
        self.signed_ms <= now_ms.saturating_add(MAX_SKEW_MS)
            && now_ms.saturating_sub(self.signed_ms) <= MAX_AGE_MS
    }
}

/// How carrying a [`Request`] to a host ended.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum Outcome {
    /// The far computer keeps the keys and listens on these relays.
    Relayed {
        #[serde(flatten)]
        info: Info,
        proof: Proof,
    },
    /// The far computer keeps the keys but listens on no relay yet.
    Kept {
        proof: Proof,
    },
    /// This host is not the computer the request names.
    Elsewhere,
    Failed {
        reason: String,
    },
}

/// The far computer's signature over its answer: its certificate
/// authority's certificate (DER), that certificate's key (P-256, SEC1
/// uncompressed), and an ECDSA P-256 SHA-256 signature, `r || s`, over
/// [`Outcome::message`], all base64url.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Proof {
    pub certificate: String,
    pub key: String,
    pub signature: String,
}

impl Proof {
    fn valid(&self) -> bool {
        !self.certificate.is_empty()
            && self.certificate.len() <= MAX_CERTIFICATE
            && field(&self.key)
            && field(&self.signature)
    }
}

impl Outcome {
    pub fn valid(&self) -> bool {
        match self {
            Self::Relayed { info, proof } => info.valid() && proof.valid(),
            Self::Kept { proof } => proof.valid(),
            Self::Failed { reason } => reason.len() <= MAX_REASON * 4,
            Self::Elsewhere => true,
        }
    }

    /// What the far computer signs for `request`: [`ANSWER_PREFIX`], the
    /// request's message, then `1` and the relays' identity, route, count
    /// (big-endian `u16`), and each relay, every text after its length as a
    /// big-endian `u16`; or `2` for keys kept without relays. `None` for an
    /// answer that claims nothing, or fields out of bounds.
    pub fn message(&self, request: &Request) -> Option<Vec<u8>> {
        let mut message = ANSWER_PREFIX.to_vec();
        message.extend(request.message()?);
        match self {
            Self::Relayed { info, .. } => {
                message.push(1);
                push_text(&mut message, &info.identity)?;
                push_text(&mut message, &info.route)?;
                message.extend_from_slice(&u16::try_from(info.relays.len()).ok()?.to_be_bytes());
                for relay in &info.relays {
                    push_text(&mut message, relay)?;
                }
            }
            Self::Kept { .. } => message.push(2),
            Self::Elsewhere | Self::Failed { .. } => return None,
        }
        Some(message)
    }

    /// Whether this answer comes from the computer `request` names: its
    /// certificate hashes to the request's fingerprint, holds the key the
    /// proof names, and that key signed [`Outcome::message`]. An answer that
    /// claims nothing proves nothing and is false too.
    pub fn proven(&self, request: &Request) -> bool {
        let (Self::Relayed { proof, .. } | Self::Kept { proof }) = self else {
            return false;
        };
        let Some(message) = self.message(request).filter(|_| self.valid()) else {
            return false;
        };
        let decode = |text: &str| URL_SAFE_NO_PAD.decode(text).ok();
        let (Some(certificate), Some(key), Some(signature)) = (
            decode(&proof.certificate),
            decode(&proof.key),
            decode(&proof.signature),
        ) else {
            return false;
        };
        let pinned = fingerprint_bytes(&request.fingerprint);
        let hashed: [u8; 32] = Sha256::digest(&certificate).into();
        // The key must be the certificate's own: a certificate that hashes
        // to the pinned fingerprint cannot be changed, and the only point
        // in it whose private key anyone holds is its owner's.
        if pinned != Some(hashed)
            || key.len() != 65
            || key[0] != 0x04
            || !certificate
                .windows(key.len())
                .any(|window| window == key.as_slice())
        {
            return false;
        }
        let (Ok(key), Ok(signature)) = (
            VerifyingKey::from_sec1_bytes(&key),
            Signature::from_slice(&signature),
        ) else {
            return false;
        };
        key.verify(&message, &signature).is_ok()
    }
}

fn push_text(message: &mut Vec<u8>, text: &str) -> Option<()> {
    message.extend_from_slice(&u16::try_from(text.len()).ok()?.to_be_bytes());
    message.extend_from_slice(text.as_bytes());
    Some(())
}

/// Where a phone's request stands on the near computer, as
/// `POST /api/v2/hosts/relay` answers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum State {
    /// Waiting for Sheprd to carry it.
    Pending,
    /// Sheprd carried it; [`Outcome::Elsewhere`] means no host this
    /// computer's Sheprd connects to is that computer.
    Done(Outcome),
}

/// A phone's request for Sheprd to carry: the phone as the near computer
/// knows it, and the host its pairing names, when the phone was paired
/// with the far computer through this one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub device: DeviceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<HostId>,
    pub request: Request,
}

/// Reads a CA fingerprint: 64 hex digits, in either case, with or without
/// `:` between pairs.
pub fn fingerprint_bytes(fingerprint: &str) -> Option<[u8; 32]> {
    let digits: Vec<u8> = fingerprint.bytes().filter(|byte| *byte != b':').collect();
    if digits.len() != 64 {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, pair) in digits.chunks(2).enumerate() {
        let text = std::str::from_utf8(pair).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(bytes)
}

fn field(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_FIELD && !value.chars().any(char::is_control)
}
