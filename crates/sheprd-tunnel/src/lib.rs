//! The end-to-end tunnel inside a relay stream (`docs/relay-plan.md`,
//! End-to-end encryption). A relay joins two byte streams and sees every
//! byte; this is what keeps it from reading, changing, or posing as either
//! side.
//!
//! - **Handshake** ([`handshake`]): `Noise_IKpsk2_25519_ChaChaPoly_SHA256`,
//!   hybrid with ML-KEM-768. The dialer knows the listener's static keys
//!   from pairing; it encapsulates to the listener's ML-KEM key inside the
//!   first message, and the shared secret is the Noise PSK, mixed in at
//!   the end of the second. The session holds if either X25519 or
//!   ML-KEM-768 does. Pairing runs the same pattern with a PSK derived
//!   from the QR's secret instead.
//! - **Records** ([`record`]): ChaCha20-Poly1305 with Noise's 64-bit
//!   counters, padded to 256-byte buckets, rekeyed every 2^20 records or
//!   ten minutes.
//! - **Framing** ([`frame`]): each handshake message and record is a
//!   big-endian `u16` length and that many bytes.
//!
//! The core is sans-I/O, so the iOS app's Swift copy follows it step for
//! step and is checked against the vectors `tests/vectors.rs` writes.
//! With the `tokio` feature, [`stream`] runs it over any Tokio stream.

mod error;
pub mod frame;
pub mod handshake;
pub mod keys;
pub mod record;
#[cfg(feature = "tokio")]
pub mod stream;

pub use error::{Error, Result};
pub use handshake::{Initiator, Pending, Responder};
pub use keys::{Identity, Keys};
pub use record::Transport;

/// The Noise protocol every handshake runs.
pub const PROTOCOL: &str = "Noise_IKpsk2_25519_ChaChaPoly_SHA256";
/// This tunnel's version, bound into every handshake's prologue.
pub const VERSION: u8 = 1;
/// The largest Noise message: a frame's length is a `u16`.
pub const MAX_MESSAGE: usize = 65_535;

#[cfg(test)]
mod tests;
