//! `sheprd-remote-control desktop-pair`: what Sheprd and the child say to
//! each other on stdio while this computer adds a nearby Sheprd over
//! Bluetooth (`docs/relay-plan.md`, Adding a nearby Sheprd from another
//! network). One JSON object per line each way.
//!
//! Sheprd carries Bluetooth: it writes each [`PairOutput::Frame`] to the
//! other computer's `pair` characteristic and hands back what that computer
//! answers as [`PairInput::Frame`]. The child does everything else: the code
//! exchange, the claim through the other computer's relays, and saving the
//! machine. Closing its input cancels.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

/// The largest frame either side carries: what the Bluetooth helper takes.
pub const MAX_FRAME: usize = 4096;
/// The longest line either side writes: a base64 frame and its envelope.
pub const MAX_LINE: usize = 8 * 1024;

/// From Sheprd to the child.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PairInput {
    /// What the other computer answered over Bluetooth, base64.
    Frame { data: String },
}

/// From the child to Sheprd.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PairOutput {
    /// A message for the other computer over Bluetooth, base64.
    Frame { data: String },
    /// The code both screens show, and the other computer's name.
    Code { code: String, computer: String },
    /// The claim reached the other computer through a relay; its user
    /// compares codes and answers there.
    Claimed,
    /// Allowed: the machine is saved and reachable as `target`.
    Paired { target: String, label: String },
    /// Why pairing stopped, for the person.
    Failed { message: String },
}

impl PairInput {
    pub fn frame(data: &[u8]) -> Self {
        Self::Frame {
            data: STANDARD.encode(data),
        }
    }
}

impl PairOutput {
    pub fn frame(data: &[u8]) -> Self {
        Self::Frame {
            data: STANDARD.encode(data),
        }
    }
}

/// A frame's bytes, when it is base64 of at most [`MAX_FRAME`] bytes.
pub fn frame_bytes(data: &str) -> Option<Vec<u8>> {
    let bytes = STANDARD.decode(data).ok()?;
    (bytes.len() <= MAX_FRAME).then_some(bytes)
}
