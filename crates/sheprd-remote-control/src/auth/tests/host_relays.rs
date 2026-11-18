//! Carrying a phone's relay keys through another computer: the near
//! computer's queue, and the far computer's check of the phone's signature.

use super::links::{devbox, enrolled, paired};
use super::*;
use crate::auth::host_relays::{Error as RelayError, TAKEN_TTL_MS};
use crate::auth::links::Link;
use crate::control::host_relay::{Info, Keys, MAX_AGE_MS, Outcome, Proof, Request, State};

/// The far computer's CA key.
fn ca() -> SigningKey {
    signing_key(77)
}

/// A stand-in for the far computer's CA certificate: bytes that hold its
/// key, as a real one does.
fn certificate() -> Vec<u8> {
    [
        b"certificate".as_slice(),
        ca().verifying_key().to_sec1_point(false).as_bytes(),
    ]
    .concat()
}

fn far_fingerprint() -> [u8; 32] {
    sha256(&[&certificate()])
}

fn far_hex() -> String {
    far_fingerprint()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

/// `outcome` for `request`, signed as the far computer signs it.
fn signed_by_far(request: &Request, outcome: Outcome) -> Outcome {
    let proof = Proof {
        certificate: encode(&certificate()),
        key: encode(ca().verifying_key().to_sec1_point(false).as_bytes()),
        signature: String::new(),
    };
    let mut outcome = match outcome {
        Outcome::Relayed { info, .. } => Outcome::Relayed { info, proof },
        _ => Outcome::Kept { proof },
    };
    let signature: Signature = ca().sign(&outcome.message(request).unwrap());
    if let Outcome::Relayed { proof, .. } | Outcome::Kept { proof } = &mut outcome {
        proof.signature = encode(&signature.to_bytes());
    }
    outcome
}

fn relayed(request: &Request) -> Outcome {
    signed_by_far(
        request,
        Outcome::Relayed {
            info: info(),
            proof: blank(),
        },
    )
}

fn kept(request: &Request) -> Outcome {
    signed_by_far(request, Outcome::Kept { proof: blank() })
}

fn blank() -> Proof {
    Proof {
        certificate: String::new(),
        key: String::new(),
        signature: String::new(),
    }
}

fn keys(seed: u8) -> Keys {
    let dial = signing_key(seed);
    Keys {
        dial: encode(dial.verifying_key().to_sec1_point(true).as_bytes()),
        tunnel: encode(&[seed; 32]),
    }
}

/// A request for `device` on the far computer, signed with `key`.
fn signed(key: &SigningKey, device: &DeviceId, keys: Keys, signed_ms: u64) -> Request {
    let mut request = Request {
        fingerprint: far_hex(),
        device: device.clone(),
        keys,
        signed_ms,
        signature: String::new(),
    };
    let signature: Signature = key.sign(&request.message().unwrap());
    request.signature = encode(&signature.to_bytes());
    request
}

fn info() -> Info {
    Info {
        identity: "identity".into(),
        labels: Default::default(),
        relays: vec!["https://relay.example.org".into()],
        route: "route".into(),
        rtt_ms: Default::default(),
    }
}

#[test]
fn the_far_computer_keeps_keys_its_own_phone_signed() {
    let dir = tempfile::tempdir().unwrap();
    let mut far = auth(dir.path());
    let key = signing_key(1);
    let device = pair(&mut far, &key, T0);
    let request = signed(&key, &device, keys(5), T0);
    assert!(
        far.keep_relay(&request, &far_fingerprint(), T0 + 1)
            .unwrap()
    );
    let stored = far.devices().get(&device).unwrap().relay.clone();
    assert_eq!(stored, Some(keys(5)));
    // The same keys again change nothing and still answer kept.
    assert!(
        far.keep_relay(&request, &far_fingerprint(), T0 + 2)
            .unwrap()
    );
    // They were saved, not only kept in memory.
    let reloaded = auth(dir.path());
    assert_eq!(
        reloaded.devices().get(&device).unwrap().relay,
        Some(keys(5))
    );
}

#[test]
fn keys_carried_late_do_not_replace_newer_ones() {
    let dir = tempfile::tempdir().unwrap();
    let mut far = auth(dir.path());
    let key = signing_key(1);
    let device = pair(&mut far, &key, T0);
    // Signed first, but still finding this computer over SSH when the
    // phone reached it directly and sent other keys.
    let late = signed(&key, &device, keys(5), T0);
    assert!(
        far.devices_mut()
            .set_relay(&device, keys(6), T0 + 60_000)
            .unwrap()
    );
    assert!(matches!(
        far.keep_relay(&late, &far_fingerprint(), T0 + 120_000),
        Err(Error::HostRelay(RelayError::Superseded))
    ));
    assert_eq!(far.devices().get(&device).unwrap().relay, Some(keys(6)));
    // Keys signed after those replace them, and are dated when kept.
    let newer = signed(&key, &device, keys(7), T0 + 90_000);
    assert!(
        far.keep_relay(&newer, &far_fingerprint(), T0 + 120_000)
            .unwrap()
    );
    let record = far.devices().get(&device).unwrap();
    assert_eq!(record.relay, Some(keys(7)));
    assert_eq!(record.relay_ms, Some(T0 + 120_000));
}

#[test]
fn another_computer_says_the_request_is_not_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut other = auth(dir.path());
    let key = signing_key(1);
    let device = pair(&mut other, &key, T0);
    let request = signed(&key, &device, keys(5), T0);
    assert!(!other.keep_relay(&request, &[0x24; 32], T0).unwrap());
    assert!(other.devices().get(&device).unwrap().relay.is_none());
}

#[test]
fn keys_signed_by_another_key_or_too_long_ago_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut far = auth(dir.path());
    let device = pair(&mut far, &signing_key(1), T0);
    // Signed by another key: what a near computer forging it would hold.
    let forged = signed(&signing_key(2), &device, keys(5), T0);
    assert!(matches!(
        far.keep_relay(&forged, &far_fingerprint(), T0),
        Err(Error::HostRelay(RelayError::Invalid))
    ));
    // Signed for one device, presented for another.
    let mut moved = signed(&signing_key(1), &device, keys(5), T0);
    moved.device = DeviceId("someone-else".into());
    assert!(far.keep_relay(&moved, &far_fingerprint(), T0).is_err());
    let stale = signed(&signing_key(1), &device, keys(5), T0);
    assert!(matches!(
        far.keep_relay(&stale, &far_fingerprint(), T0 + MAX_AGE_MS + 1),
        Err(Error::HostRelay(RelayError::Stale))
    ));
    // Keys that are not keys, however well signed.
    let bad = signed(
        &signing_key(1),
        &device,
        Keys {
            dial: encode(&[7; 33]),
            tunnel: encode(&[7; 32]),
        },
        T0,
    );
    assert!(matches!(
        far.keep_relay(&bad, &far_fingerprint(), T0),
        Err(Error::HostRelay(RelayError::Invalid))
    ));
    assert!(far.devices().get(&device).unwrap().relay.is_none());
}

#[test]
fn an_unknown_device_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut far = auth(dir.path());
    let request = signed(&signing_key(1), &DeviceId("gone".into()), keys(5), T0);
    assert!(matches!(
        far.keep_relay(&request, &far_fingerprint(), T0),
        Err(Error::UnknownDevice)
    ));
}

#[test]
fn the_near_computer_queues_a_request_until_sheprd_carries_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut near = auth(dir.path());
    let phone = pair(&mut near, &signing_key(1), T0);
    let request = signed(&signing_key(9), &DeviceId("far-id".into()), keys(5), T0);
    let state = near.ask_host_relay(&phone, request.clone(), T0).unwrap();
    assert_eq!(state, State::Pending);
    let jobs = near.take_host_relays(T0);
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].device, phone);
    assert_eq!(jobs[0].request, request);
    assert!(jobs[0].host.is_none());
    assert!(near.take_host_relays(T0).is_empty());
    // Posting the same request again reads where it stands.
    assert_eq!(
        near.ask_host_relay(&phone, request.clone(), T0).unwrap(),
        State::Pending
    );
    near.host_relayed(&phone, &request, relayed(&request), T0 + 5)
        .unwrap();
    assert_eq!(
        near.ask_host_relay(&phone, request.clone(), T0 + 6)
            .unwrap(),
        State::Done(relayed(&request))
    );
}

#[test]
fn new_keys_wait_while_old_ones_are_being_carried() {
    let dir = tempfile::tempdir().unwrap();
    let mut near = auth(dir.path());
    let phone = pair(&mut near, &signing_key(1), T0);
    let far = DeviceId("far-id".into());
    let first = signed(&signing_key(9), &far, keys(5), T0);
    near.ask_host_relay(&phone, first.clone(), T0).unwrap();
    near.take_host_relays(T0);
    let second = signed(&signing_key(9), &far, keys(6), T0 + 1);
    assert!(matches!(
        near.ask_host_relay(&phone, second.clone(), T0 + 1),
        Err(Error::HostRelay(RelayError::Busy))
    ));
    // An answer for keys the phone did not ask about is refused.
    assert!(matches!(
        near.host_relayed(&phone, &second, kept(&second), T0 + 2),
        Err(Error::HostRelay(RelayError::NotCarrying))
    ));
    near.host_relayed(&phone, &first, kept(&first), T0 + 2)
        .unwrap();
    // Once answered, new keys replace the old request.
    assert_eq!(
        near.ask_host_relay(&phone, second, T0 + 3).unwrap(),
        State::Pending
    );
}

#[test]
fn a_request_sheprd_never_finishes_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut near = auth(dir.path());
    let phone = pair(&mut near, &signing_key(1), T0);
    let request = signed(&signing_key(9), &DeviceId("far-id".into()), keys(5), T0);
    near.ask_host_relay(&phone, request.clone(), T0).unwrap();
    near.take_host_relays(T0);
    let state = near
        .ask_host_relay(&phone, request, T0 + TAKEN_TTL_MS)
        .unwrap();
    assert!(matches!(state, State::Done(Outcome::Failed { .. })));
}

#[test]
fn requests_out_of_bounds_and_answers_out_of_bounds_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut near = auth(dir.path());
    let phone = pair(&mut near, &signing_key(1), T0);
    let mut bad = signed(&signing_key(9), &DeviceId("far-id".into()), keys(5), T0);
    bad.fingerprint = "12:34".into();
    assert!(matches!(
        near.ask_host_relay(&phone, bad, T0),
        Err(Error::HostRelay(RelayError::Invalid))
    ));
    let request = signed(&signing_key(9), &DeviceId("far-id".into()), keys(5), T0);
    near.ask_host_relay(&phone, request.clone(), T0).unwrap();
    near.take_host_relays(T0);
    let mut empty = relayed(&request);
    if let Outcome::Relayed { info, .. } = &mut empty {
        info.relays.clear();
    }
    assert!(near.host_relayed(&phone, &request, empty, T0).is_err());
    // An answer the far computer did not sign, or signed for other keys.
    let mut unsigned = relayed(&request);
    if let Outcome::Relayed { proof, .. } = &mut unsigned {
        proof.signature = encode(&[1; 64]);
    }
    assert!(matches!(
        near.host_relayed(&phone, &request, unsigned, T0),
        Err(Error::HostRelay(RelayError::Invalid))
    ));
    let other = signed(&signing_key(9), &DeviceId("far-id".into()), keys(6), T0);
    assert!(
        near.host_relayed(&phone, &request, relayed(&other), T0)
            .is_err()
    );
    assert!(matches!(
        near.ask_host_relay(&DeviceId("stranger".into()), request, T0),
        Err(Error::UnknownDevice)
    ));
}

#[test]
fn a_phone_paired_with_the_far_computer_through_this_one_names_its_host() {
    let dir = tempfile::tempdir().unwrap();
    let (mut near, phone) = paired(dir.path(), &[devbox()]);
    let mut answer = enrolled("far-id");
    answer.fingerprint = far_hex();
    near.devices_mut()
        .set_link(
            &phone,
            Link {
                host: devbox(),
                name: "devbox".into(),
                paired: Some(answer),
            },
        )
        .unwrap();
    let request = signed(&signing_key(1), &DeviceId("far-id".into()), keys(5), T0);
    near.ask_host_relay(&phone, request, T0).unwrap();
    let jobs = near.take_host_relays(T0);
    assert_eq!(jobs[0].host, Some(devbox()));
}

#[test]
fn revoking_the_phone_ends_its_requests() {
    let dir = tempfile::tempdir().unwrap();
    let mut near = auth(dir.path());
    let phone = pair(&mut near, &signing_key(1), T0);
    let request = signed(&signing_key(9), &DeviceId("far-id".into()), keys(5), T0);
    near.ask_host_relay(&phone, request, T0).unwrap();
    near.revoke(&phone).unwrap();
    assert!(near.take_host_relays(T0).is_empty());
}
