//! A desktop adds a nearby computer with no network path between them:
//! the code exchange as Bluetooth would carry it ([`exchange`]), the
//! records kept ([`record`]), and the whole flow through a real relay on
//! loopback ([`flow`]).

use super::*;
use crate::{
    control::{NearbyRevealed, NearbyStarted},
    web::{Shared, now_ms, tests::state},
};

mod exchange;
#[cfg(unix)]
mod flow;
mod record;

/// What the other computer's Sheprd does with a frame it reads over
/// Bluetooth: the control socket's nearby steps, answered as the station
/// writes them back.
fn bluetooth(state: &Shared, frame: &[u8]) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case", tag = "type")]
    enum Message {
        Start { public_key: String },
        Reveal { session: String, nonce: String },
    }
    let now = now_ms();
    let answer = match serde_json::from_slice(frame).unwrap() {
        Message::Start { public_key } => serde_json::to_vec::<NearbyStarted>(
            &state
                .auth()
                .nearby_start(None, &public_key, &state.computer, &state.fingerprint, now)
                .unwrap(),
        ),
        Message::Reveal { session, nonce } => {
            let origins = state.origins();
            let revealed = {
                let mut auth = state.auth();
                crate::relay::refresh_offer(state.relay.as_deref(), &mut auth);
                auth.nearby_reveal(&session, &nonce, &state.fingerprint, &origins, now)
                    .unwrap()
            };
            crate::relay::open_pairing(state);
            serde_json::to_vec::<NearbyRevealed>(&revealed)
        }
    };
    answer.unwrap()
}
