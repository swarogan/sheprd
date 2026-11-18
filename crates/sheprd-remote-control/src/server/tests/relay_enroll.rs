//! `relay-enroll`, which Sheprd on another computer runs here over SSH to
//! carry a phone's relay keys: its command line, and the daemon's answer
//! over its control socket.

use super::*;
use crate::control::{
    DeviceId, Request, Response,
    host_relay::{self, Keys, Outcome},
    hosts::Enroll,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};

#[test]
fn relay_enroll_takes_only_a_state_directory() {
    assert_eq!(
        parse(args(&["relay-enroll"])).unwrap(),
        Invocation::RelayEnroll { state_dir: None }
    );
    assert_eq!(
        parse(args(&["relay-enroll", "--state-dir", "/s"])).unwrap(),
        Invocation::RelayEnroll {
            state_dir: Some("/s".into())
        }
    );
    assert_eq!(
        parse(args(&["relay-enroll", "--offer"])).unwrap_err(),
        ParseError::Unknown("--offer".into())
    );
    assert_eq!(
        parse(args(&["relay-enroll", "--help"])).unwrap(),
        Invocation::Help
    );
}

fn signed(key: &SigningKey, fingerprint: &str, device: &DeviceId) -> host_relay::Request {
    let dial = SigningKey::from_slice(&[3; 32]).unwrap();
    let mut request = host_relay::Request {
        fingerprint: fingerprint.into(),
        device: device.clone(),
        keys: Keys {
            dial: crate::auth::encode(dial.verifying_key().to_sec1_point(true).as_bytes()),
            tunnel: crate::auth::encode(&[4; 32]),
        },
        signed_ms: web::now_ms(),
        signature: String::new(),
    };
    let signature: Signature = key.sign(&request.message().unwrap());
    request.signature = crate::auth::encode(&signature.to_bytes());
    request
}

#[test]
fn the_daemon_keeps_relay_keys_its_phone_signed_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let state = web::tests::state(dir.path());
    let (stop, _) = watch::channel(false);
    let key = SigningKey::from_slice(&[7; 32]).unwrap();
    let Response::Enrolled(enrolled) = control::answer(
        &state,
        &stop,
        Request::Enroll(Enroll {
            name: "iPhone".into(),
            public_key: crate::auth::encode(key.verifying_key().to_sec1_point(false).as_bytes()),
            via: "MacBook Pro".into(),
            replace: None,
        }),
    ) else {
        panic!("not enrolled");
    };
    let request = signed(&key, &state.fingerprint, &enrolled.device);
    // This computer listens on no relay: the keys wait for one. The answer
    // is signed with its CA, as the phone pinned it.
    let Response::Relayed(outcome @ Outcome::Kept { .. }) = control::answer(
        &state,
        &stop,
        Request::RelayEnroll(Box::new(request.clone())),
    ) else {
        panic!("not kept");
    };
    assert!(outcome.proven(&request));
    assert!(
        state
            .auth()
            .devices()
            .get(&enrolled.device)
            .unwrap()
            .relay
            .is_some()
    );
    let elsewhere = signed(&key, &"AB".repeat(32), &enrolled.device);
    assert_eq!(
        control::answer(&state, &stop, Request::RelayEnroll(Box::new(elsewhere))),
        Response::Relayed(Outcome::Elsewhere)
    );
    let forged = signed(
        &SigningKey::from_slice(&[8; 32]).unwrap(),
        &state.fingerprint,
        &enrolled.device,
    );
    assert!(matches!(
        control::answer(&state, &stop, Request::RelayEnroll(Box::new(forged))),
        Response::Relayed(Outcome::Failed { .. })
    ));
}
