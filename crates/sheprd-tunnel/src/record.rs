//! Records after the handshake: ChaCha20-Poly1305 under Noise's transport
//! keys, 64-bit counters, padding, and rekeying.
//!
//! A record's plaintext is `flags ‖ length (u16, big-endian) ‖ data ‖
//! zeros`, padded to a multiple of [`PAD`] bytes, so a keystroke and a
//! short reply look alike from outside. A sender sets [`REKEY`] on the
//! record after which it rekeys (every [`REKEY_RECORDS`] records or
//! [`REKEY_AFTER`]); the receiver rekeys after opening it, so both stay in
//! step without a message of their own. [`CLOSE`] ends the tunnel, so a
//! relay cutting the stream is told apart from the other side leaving.

use crate::{Error, MAX_MESSAGE, Result};
use snow::TransportState;
use std::time::{Duration, Instant};

/// Records are padded to a multiple of this many bytes.
pub const PAD: usize = 256;
/// The most data one record carries.
pub const MAX_DATA: usize = 16 * 1024;
/// The sender rekeys after this record.
pub const REKEY: u8 = 0b01;
/// The sender is done; nothing follows.
pub const CLOSE: u8 = 0b10;
/// Records sent between rekeys, at most.
pub const REKEY_RECORDS: u64 = 1 << 20;
/// Time between rekeys, at most.
pub const REKEY_AFTER: Duration = Duration::from_secs(10 * 60);
/// The flags, then the data's length.
const HEADER: usize = 3;
/// Poly1305's tag.
const TAG: usize = 16;

/// What one record held.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Opened {
    Data(Vec<u8>),
    /// The other side closed the tunnel.
    Closed,
}

/// The tunnel's two directions, after the handshake.
#[derive(Debug)]
pub struct Transport {
    /// When this side last rekeyed what it sends, or began sending.
    rekeyed: Option<Instant>,
    /// Records sent since then.
    sent: u64,
    state: TransportState,
}

impl Transport {
    pub(crate) fn new(state: TransportState) -> Self {
        Self {
            rekeyed: None,
            sent: 0,
            state,
        }
    }

    /// Seals `data` (at most [`MAX_DATA`] bytes) as one record, rekeying
    /// after it when due at `now`.
    pub fn seal(&mut self, data: &[u8], now: Instant) -> Result<Vec<u8>> {
        if data.len() > MAX_DATA {
            return Err(Error::FrameTooLong);
        }
        let since = *self.rekeyed.get_or_insert(now);
        let due =
            self.sent + 1 >= REKEY_RECORDS || now.saturating_duration_since(since) >= REKEY_AFTER;
        let flags = if due { REKEY } else { 0 };
        let record = self.seal_flags(flags, data)?;
        if due {
            self.state.rekey_outgoing();
            self.sent = 0;
            self.rekeyed = Some(now);
        } else {
            self.sent += 1;
        }
        Ok(record)
    }

    /// Seals the record that closes the tunnel.
    pub fn seal_close(&mut self) -> Result<Vec<u8>> {
        self.seal_flags(CLOSE, &[])
    }

    fn seal_flags(&mut self, flags: u8, data: &[u8]) -> Result<Vec<u8>> {
        let length = u16::try_from(data.len()).map_err(|_| Error::FrameTooLong)?;
        let padded = (HEADER + data.len()).div_ceil(PAD) * PAD;
        let mut plain = Vec::with_capacity(padded);
        plain.push(flags);
        plain.extend_from_slice(&length.to_be_bytes());
        plain.extend_from_slice(data);
        plain.resize(padded, 0);
        let mut record = vec![0; padded + TAG];
        let written = self
            .state
            .write_message(&plain, &mut record)
            .map_err(Error::Record)?;
        record.truncate(written);
        Ok(record)
    }

    /// Opens one record. Anything changed, replayed, reordered, or padded
    /// with other than zeros is refused, and the tunnel is then unusable.
    pub fn open(&mut self, record: &[u8]) -> Result<Opened> {
        if record.len() > MAX_MESSAGE || record.len() < TAG + PAD {
            return Err(Error::Malformed);
        }
        let mut plain = vec![0; record.len()];
        let length = self
            .state
            .read_message(record, &mut plain)
            .map_err(Error::Record)?;
        plain.truncate(length);
        let [flags, high, low, rest @ ..] = plain.as_slice() else {
            return Err(Error::Malformed);
        };
        let data_len = usize::from(u16::from_be_bytes([*high, *low]));
        let known = REKEY | CLOSE;
        if flags & !known != 0
            || !plain.len().is_multiple_of(PAD)
            || data_len > rest.len()
            || data_len > MAX_DATA
            || rest[data_len..].iter().any(|byte| *byte != 0)
        {
            return Err(Error::Malformed);
        }
        if flags & CLOSE != 0 {
            return Ok(Opened::Closed);
        }
        let data = rest[..data_len].to_vec();
        if flags & REKEY != 0 {
            self.state.rekey_incoming();
        }
        Ok(Opened::Data(data))
    }
}
