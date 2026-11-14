//! Framing on the relay's byte stream: each handshake message and record
//! is a big-endian `u16` length, then that many bytes.

use crate::{Error, MAX_MESSAGE, Result};

/// A frame's length prefix.
pub const PREFIX: usize = 2;

/// `message` as one frame.
pub fn encode(message: &[u8]) -> Result<Vec<u8>> {
    let length = u16::try_from(message.len()).map_err(|_| Error::FrameTooLong)?;
    Ok([&length.to_be_bytes()[..], message].concat())
}

/// Collects bytes as they arrive and gives back whole frames. It holds at
/// most one frame and what arrived after it, so a peer sending without
/// pause cannot grow it past what the caller reads at once.
#[derive(Debug, Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    /// Adds bytes read from the stream.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, if one has arrived.
    #[must_use]
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        let [high, low, ..] = self.buffer.as_slice() else {
            return None;
        };
        let length = usize::from(u16::from_be_bytes([*high, *low]));
        if self.buffer.len() < PREFIX + length {
            return None;
        }
        let frame = self.buffer[PREFIX..PREFIX + length].to_vec();
        self.buffer.drain(..PREFIX + length);
        Some(frame)
    }

    /// How many more bytes the frame being collected needs, at least.
    #[must_use]
    pub fn wanted(&self) -> usize {
        match self.buffer.as_slice() {
            [high, low, ..] => {
                let length = usize::from(u16::from_be_bytes([*high, *low]));
                (PREFIX + length).saturating_sub(self.buffer.len()).max(1)
            }
            _ => PREFIX - self.buffer.len(),
        }
    }

    /// Bytes held, for bounds checks.
    #[must_use]
    pub fn held(&self) -> usize {
        self.buffer.len()
    }
}

/// The most a decoder ever needs to hold for one frame.
pub const MAX_FRAME: usize = PREFIX + MAX_MESSAGE;
