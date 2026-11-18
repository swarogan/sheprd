//! The phone's side of the code exchange against the daemon's own, with
//! no relay: the same code on both screens, and a commitment that does not
//! match refused.

use super::*;
pub(super) use crate::desktop::exchange::Exchange;

/// The daemon's side straight through `Auth`, with Pair device open.
fn daemon() -> (tempfile::TempDir, Shared) {
    let dir = tempfile::tempdir().unwrap();
    let state = state(dir.path());
    state
        .auth()
        .mint(
            now_ms(),
            &[crate::web::tests::ORIGIN.to_owned()],
            &state.fingerprint,
        )
        .unwrap();
    (dir, state)
}

#[tokio::test]
async fn both_sides_agree_on_the_code_and_the_sealed_token() {
    let (_dir, state) = daemon();
    let (mut exchange, start) = Exchange::start().unwrap();
    let reveal = exchange.reveal(&bluetooth(&state, &start)).unwrap();
    let opened = exchange.open(&bluetooth(&state, &reveal)).unwrap();
    assert_eq!(opened.code.len(), 6);
    assert_eq!(
        opened.fingerprint.to_vec(),
        crate::auth::nearby::fingerprint_bytes(&state.fingerprint)
            .unwrap()
            .to_vec()
    );
    // No relays here: the offer says so, and claiming needs them.
    assert!(opened.relay.is_none());
}

#[tokio::test]
async fn a_nonce_the_daemon_did_not_commit_to_is_refused() {
    let (_dir, state) = daemon();
    let (mut exchange, start) = Exchange::start().unwrap();
    let reveal = exchange.reveal(&bluetooth(&state, &start)).unwrap();
    let mut revealed: serde_json::Value =
        serde_json::from_slice(&bluetooth(&state, &reveal)).unwrap();
    revealed["nonce"] = crate::auth::encode(&[7; 32]).into();
    let result = exchange.open(revealed.to_string().as_bytes());
    assert!(
        matches!(result, Err(Error::Nearby(crate::auth::Error::NearbyCrypto))),
        "{result:?}"
    );
}

#[test]
fn an_error_answer_is_reported_in_the_other_computers_words() {
    let (mut exchange, _) = Exchange::start().unwrap();
    let result =
        exchange.reveal(br#"{"error":"open Remote control > Pair device on the computer first"}"#);
    assert!(
        matches!(&result, Err(Error::NearbyRefused(reason)) if reason.contains("Pair device")),
        "{result:?}"
    );
}
