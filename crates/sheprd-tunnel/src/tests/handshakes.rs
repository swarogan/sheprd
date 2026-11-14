//! Both handshakes end to end, and what each refuses.

use super::*;
use crate::{
    Error, Identity, Initiator, Responder,
    handshake::{Mode, pairing_code, prologue},
    keys::IDENTITY_LEN,
    record::Opened,
};
use std::time::Instant;

#[test]
fn a_paired_device_opens_a_session_both_ways() {
    let (phone, computer) = (keys(1), keys(2));
    let (mut dialing, mut listening) = session(&phone, &computer);
    let now = Instant::now();
    let record = dialing.seal(b"GET /api/v2/chats", now).unwrap();
    assert_eq!(
        listening.open(&record).unwrap(),
        Opened::Data(b"GET /api/v2/chats".to_vec())
    );
    let reply = listening.seal(b"200", now).unwrap();
    assert_eq!(dialing.open(&reply).unwrap(), Opened::Data(b"200".to_vec()));
}

#[test]
fn the_wrong_listener_cannot_answer_a_session() {
    let (phone, computer, impostor) = (keys(1), keys(2), keys(3));
    // The phone pinned the computer; the impostor holds the route.
    let (_, first) = Initiator::session(&phone, &computer.identity(), &ROUTE).unwrap();
    let responder = Responder::session(&impostor, &ROUTE).unwrap();
    assert!(matches!(
        responder.read(&impostor, &first),
        Err(Error::Handshake(_))
    ));
}

#[test]
fn a_session_is_bound_to_its_route() {
    let (phone, computer) = (keys(1), keys(2));
    let (_, first) = Initiator::session(&phone, &computer.identity(), &ROUTE).unwrap();
    let elsewhere = Responder::session(&computer, &[0x99; 32]).unwrap();
    assert!(elsewhere.read(&computer, &first).is_err());
}

#[test]
fn a_changed_answer_or_first_message_is_refused() {
    let (phone, computer) = (keys(1), keys(2));
    let (initiator, mut first) = Initiator::session(&phone, &computer.identity(), &ROUTE).unwrap();
    let last = first.len() - 1;
    first[last] ^= 1;
    let responder = Responder::session(&computer, &ROUTE).unwrap();
    assert!(responder.read(&computer, &first).is_err());
    first[last] ^= 1;
    let responder = Responder::session(&computer, &ROUTE).unwrap();
    let (mut answer, _, _) = responder.read(&computer, &first).unwrap().accept().unwrap();
    answer[0] ^= 1;
    assert!(initiator.finish(&answer).is_err());
}

#[test]
fn the_ml_kem_secret_is_part_of_the_session() {
    // The same X25519 key and prologue, but a listener that opens the
    // ciphertext with another ML-KEM key: it derives another PSK, and the
    // dialer refuses its answer. Breaking X25519 alone is not enough.
    let (phone, computer) = (keys(1), keys(2));
    let (x25519, _) = computer.secrets();
    let other_kem = Keys::from_secrets(x25519, [0xee; 64]);
    let (initiator, first) = Initiator::session(&phone, &computer.identity(), &ROUTE).unwrap();
    let responder = Responder::session(&computer, &ROUTE).unwrap();
    let pending = responder.read(&other_kem, &first).unwrap();
    let (answer, _, _) = pending.accept().unwrap();
    assert!(matches!(
        initiator.finish(&answer),
        Err(Error::Handshake(_))
    ));
}

#[test]
fn pairing_needs_the_qr_secret_and_both_sides_see_one_code() {
    let (phone, computer) = (keys(1), keys(2));
    let secret = [0x5e; 32];
    let identity = computer.identity();
    let (initiator, first) = Initiator::pairing(
        &phone,
        &identity.x25519(),
        &identity.fingerprint(),
        &ROUTE,
        &secret,
    )
    .unwrap();
    let pending = Responder::pairing(&computer, &ROUTE, &secret)
        .unwrap()
        .read(&computer, &first)
        .unwrap();
    assert_eq!(pending.remote(), phone.identity().x25519());
    let (answer, _, listener_hash) = pending.accept().unwrap();
    let (_, dialer_hash) = initiator.finish(&answer).unwrap();
    assert_eq!(pairing_code(&listener_hash), pairing_code(&dialer_hash));
    assert!(pairing_code(&dialer_hash) < 1_000_000);

    // Someone without the secret gets nowhere.
    let (stranger, first) = Initiator::pairing(
        &keys(4),
        &identity.x25519(),
        &identity.fingerprint(),
        &ROUTE,
        &[0x00; 32],
    )
    .unwrap();
    let pending = Responder::pairing(&computer, &ROUTE, &secret)
        .unwrap()
        .read(&computer, &first);
    match pending {
        Err(_) => {}
        Ok(pending) => {
            let (answer, _, _) = pending.accept().unwrap();
            assert!(stranger.finish(&answer).is_err());
        }
    }
}

#[test]
fn a_pairing_message_is_not_a_session_message() {
    let (phone, computer) = (keys(1), keys(2));
    let identity = computer.identity();
    let (_, first) = Initiator::pairing(
        &phone,
        &identity.x25519(),
        &identity.fingerprint(),
        &ROUTE,
        &[0x5e; 32],
    )
    .unwrap();
    let responder = Responder::session(&computer, &ROUTE).unwrap();
    assert!(responder.read(&computer, &first).is_err());
}

#[test]
fn identities_round_trip_and_refuse_nonsense() {
    let identity = keys(7).identity();
    let bytes = identity.to_bytes();
    assert_eq!(bytes.len(), IDENTITY_LEN);
    assert_eq!(Identity::from_bytes(&bytes).unwrap(), identity);
    assert!(Identity::from_bytes(&bytes[1..]).is_err());
    let restored = Keys::from_secrets(keys(7).secrets().0, keys(7).secrets().1);
    assert_eq!(restored.identity(), identity);
    assert_ne!(keys(8).identity().fingerprint(), identity.fingerprint());
    // The prologue names the mode, so the same route and listener differ.
    let fingerprint = identity.fingerprint();
    assert_ne!(
        prologue(Mode::Session, &ROUTE, &fingerprint),
        prologue(Mode::Pairing, &ROUTE, &fingerprint)
    );
}
