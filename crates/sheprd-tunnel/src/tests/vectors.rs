//! The vectors the iOS app's Swift tunnel is checked against: every key
//! and random value fixed, and what each step must produce, byte for byte.
//! `HERDR_UPDATE_FIXTURES=1` rewrites them.

use super::*;
use crate::{
    Initiator, Responder,
    handshake::{Fixed, Mode, pairing_code, pairing_psk, prologue},
    record::REKEY_AFTER,
};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Instant};

const DIALER_EPHEMERAL: [u8; 32] = [0x11; 32];
const LISTENER_EPHEMERAL: [u8; 32] = [0x22; 32];
const ENCAPSULATION: [u8; 32] = [0x33; 32];
const SECRET: [u8; 32] = [0x5e; 32];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn check(name: &str, value: &Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ios/SheprdRemoteCore/Tests/SheprdRemoteCoreTests/Fixtures/tunnel")
        .join(format!("{name}.json"));
    let text = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var_os("HERDR_UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let expected: Value = serde_json::from_str(&expected).unwrap();
    assert_eq!(
        value,
        &expected,
        "{} is out of date; run with HERDR_UPDATE_FIXTURES=1",
        path.display()
    );
}

/// Records both ways, a rekey among them, then a close.
fn records(dialing: &mut Transport, listening: &mut Transport) -> Value {
    let start = Instant::now();
    let steps: [(&str, &[u8], Instant); 5] = [
        ("dialer", b"GET /api/v2/chats HTTP/1.1\r\n\r\n", start),
        ("listener", b"HTTP/1.1 200 OK\r\n\r\n", start),
        ("dialer", &[0x41; 300], start),
        // Sealed ten minutes on: carries the rekey flag.
        ("dialer", b"rekeyed after this", start + REKEY_AFTER),
        ("dialer", b"under the new key", start + REKEY_AFTER),
    ];
    let mut out = Vec::new();
    for (from, data, at) in steps {
        let (sender, receiver) = if from == "dialer" {
            (&mut *dialing, &mut *listening)
        } else {
            (&mut *listening, &mut *dialing)
        };
        let record = sender.seal(data, at).unwrap();
        assert_eq!(
            receiver.open(&record).unwrap(),
            crate::record::Opened::Data(data.to_vec())
        );
        out.push(json!({"from": from, "data": hex(data), "record": hex(&record)}));
    }
    let close = dialing.seal_close().unwrap();
    assert_eq!(
        listening.open(&close).unwrap(),
        crate::record::Opened::Closed
    );
    out.push(json!({"from": "dialer", "close": true, "record": hex(&close)}));
    Value::Array(out)
}

#[test]
fn session_vectors() {
    let (dialer, listener) = (keys(1), keys(2));
    let identity = listener.identity();
    let fixed_dialer = Fixed {
        ephemeral: Some(DIALER_EPHEMERAL),
        encapsulation: Some(ENCAPSULATION),
    };
    let fixed_listener = Fixed {
        ephemeral: Some(LISTENER_EPHEMERAL),
        encapsulation: None,
    };
    let (ciphertext, shared) = identity.encapsulate(&ENCAPSULATION).unwrap();
    let (initiator, first) =
        Initiator::session_fixed(&dialer, &identity, &ROUTE, &fixed_dialer).unwrap();
    let pending = Responder::new(&listener, Mode::Session, &ROUTE, None, &fixed_listener)
        .unwrap()
        .read(&listener, &first)
        .unwrap();
    let (answer, mut listening, listener_hash) = pending.accept().unwrap();
    let (mut dialing, hash) = initiator.finish(&answer).unwrap();
    assert_eq!(hash, listener_hash);
    let (dialer_x25519, _) = dialer.secrets();
    let (listener_x25519, listener_seed) = listener.secrets();
    check(
        "session",
        &json!({
            "protocol": crate::PROTOCOL,
            "dialer_static": hex(&dialer_x25519),
            "dialer_public": hex(&dialer.identity().x25519()),
            "dialer_ephemeral": hex(&DIALER_EPHEMERAL),
            "listener_static": hex(&listener_x25519),
            "listener_mlkem_seed": hex(&listener_seed),
            "listener_ephemeral": hex(&LISTENER_EPHEMERAL),
            "listener_identity": hex(&identity.to_bytes()),
            "listener_fingerprint": hex(&identity.fingerprint()),
            "route": hex(&ROUTE),
            "prologue": hex(&prologue(Mode::Session, &ROUTE, &identity.fingerprint())),
            "encapsulation_randomness": hex(&ENCAPSULATION),
            "mlkem_ciphertext": hex(&ciphertext),
            "mlkem_shared": hex(&shared),
            "first": hex(&first),
            "answer": hex(&answer),
            "handshake_hash": hex(&hash),
            "records": records(&mut dialing, &mut listening),
        }),
    );
}

#[test]
fn pairing_vectors() {
    let (dialer, listener) = (keys(1), keys(2));
    let identity = listener.identity();
    let fixed_dialer = Fixed {
        ephemeral: Some(DIALER_EPHEMERAL),
        encapsulation: None,
    };
    let fixed_listener = Fixed {
        ephemeral: Some(LISTENER_EPHEMERAL),
        encapsulation: None,
    };
    let (initiator, first) = Initiator::pairing_fixed(
        &dialer,
        (&identity.x25519(), &identity.fingerprint()),
        &ROUTE,
        &SECRET,
        &fixed_dialer,
    )
    .unwrap();
    let pending = Responder::new(
        &listener,
        Mode::Pairing,
        &ROUTE,
        Some(&SECRET),
        &fixed_listener,
    )
    .unwrap()
    .read(&listener, &first)
    .unwrap();
    let (answer, mut listening, _) = pending.accept().unwrap();
    let (mut dialing, hash) = initiator.finish(&answer).unwrap();
    let (dialer_x25519, _) = dialer.secrets();
    check(
        "pairing",
        &json!({
            "dialer_static": hex(&dialer_x25519),
            "dialer_ephemeral": hex(&DIALER_EPHEMERAL),
            "listener_public": hex(&identity.x25519()),
            "listener_fingerprint": hex(&identity.fingerprint()),
            "listener_ephemeral": hex(&LISTENER_EPHEMERAL),
            "route": hex(&ROUTE),
            "secret": hex(&SECRET),
            "psk": hex(&pairing_psk(&SECRET)),
            "prologue": hex(&prologue(Mode::Pairing, &ROUTE, &identity.fingerprint())),
            "first": hex(&first),
            "answer": hex(&answer),
            "handshake_hash": hex(&hash),
            "code": pairing_code(&hash),
            "records": records(&mut dialing, &mut listening),
        }),
    );
}
