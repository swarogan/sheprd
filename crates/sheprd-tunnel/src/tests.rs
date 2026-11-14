#![allow(clippy::unwrap_used, clippy::expect_used)]

mod frames;
mod handshakes;
mod records;
mod vectors;

use crate::{Keys, Transport};

/// A computer's or phone's keys, from fixed secrets.
pub(super) fn keys(seed: u8) -> Keys {
    Keys::from_secrets([seed; 32], [seed.wrapping_add(1); 64])
}

pub(super) const ROUTE: [u8; 32] = [0x52; 32];

/// A session tunnel between `dialer` and `listener`, both ends.
pub(super) fn session(dialer: &Keys, listener: &Keys) -> (Transport, Transport) {
    let responder = crate::Responder::session(listener, &ROUTE).unwrap();
    let (initiator, first) =
        crate::Initiator::session(dialer, &listener.identity(), &ROUTE).unwrap();
    let pending = responder.read(listener, &first).unwrap();
    assert_eq!(pending.remote(), dialer.identity().x25519());
    let (answer, listening, _) = pending.accept().unwrap();
    let (dialing, _) = initiator.finish(&answer).unwrap();
    (dialing, listening)
}
#[cfg(feature = "tokio")]
mod streams;
