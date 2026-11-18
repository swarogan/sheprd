//! The wire of carrying a phone's relay keys through another computer: what
//! the phone signs, how long a signature lasts, and the bounds on answers.

use super::*;
use crate::control::host_relay::{
    self, ANSWER_PREFIX, Info, Job, Keys, MAX_AGE_MS, MAX_FIELD, MAX_RELAYS, MAX_SKEW_MS, Outcome,
    PREFIX, Proof, State, fingerprint_bytes,
};

const FINGERPRINT: &str = "00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff";

fn request() -> host_relay::Request {
    host_relay::Request {
        fingerprint: FINGERPRINT.into(),
        device: DeviceId("dev".into()),
        keys: Keys {
            dial: "DIAL".into(),
            tunnel: "TUN".into(),
        },
        signed_ms: 0x0102_0304_0506_0708,
        signature: "SIG".into(),
    }
}

fn proof() -> Proof {
    Proof {
        certificate: "CERT".into(),
        key: "KEY".into(),
        signature: "SIG".into(),
    }
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
fn the_signed_message_is_the_prefix_fingerprint_length_prefixed_fields_and_time() {
    let mut expected = PREFIX.to_vec();
    expected.extend(
        (0..=0xff_u8)
            .step_by(0x11)
            .chain((0..=0xff_u8).step_by(0x11)),
    );
    expected.extend([0, 3]);
    expected.extend(b"dev");
    expected.extend([0, 4]);
    expected.extend(b"DIAL");
    expected.extend([0, 3]);
    expected.extend(b"TUN");
    expected.extend([1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(request().message().unwrap(), expected);
}

#[test]
fn a_request_out_of_bounds_has_no_message() {
    let mut bad = request();
    bad.fingerprint = "00:11".into();
    assert!(bad.message().is_none());
    let mut bad = request();
    bad.keys.dial = String::new();
    assert!(bad.message().is_none());
    let mut bad = request();
    bad.device = DeviceId("a\nb".into());
    assert!(bad.message().is_none());
    // The signature is not signed, but a request needs one in bounds.
    let mut unsigned = request();
    unsigned.signature = String::new();
    assert!(unsigned.message().is_some());
    assert!(!unsigned.valid());
    unsigned.signature = "x".repeat(MAX_FIELD + 1);
    assert!(!unsigned.valid());
}

#[test]
fn fingerprints_read_with_or_without_colons_in_either_case() {
    let plain = FINGERPRINT.replace(':', "").to_lowercase();
    assert_eq!(fingerprint_bytes(FINGERPRINT), fingerprint_bytes(&plain));
    assert!(fingerprint_bytes(&plain).is_some());
    assert!(fingerprint_bytes("zz").is_none());
    assert!(fingerprint_bytes(&"g".repeat(64)).is_none());
}

#[test]
fn a_signature_lasts_a_day_and_may_run_a_little_ahead() {
    let mut request = request();
    let now = 10 * MAX_AGE_MS;
    request.signed_ms = now - MAX_AGE_MS;
    assert!(request.fresh(now));
    request.signed_ms = now - MAX_AGE_MS - 1;
    assert!(!request.fresh(now));
    request.signed_ms = now + MAX_SKEW_MS;
    assert!(request.fresh(now));
    request.signed_ms = now + MAX_SKEW_MS + 1;
    assert!(!request.fresh(now));
}

#[test]
fn relay_info_is_bounded() {
    assert!(info().valid());
    let mut none = info();
    none.relays.clear();
    assert!(!none.valid());
    let mut many = info();
    many.relays = vec!["https://relay.example.org".into(); MAX_RELAYS + 1];
    assert!(!many.valid());
    let mut long = info();
    long.identity = "x".repeat(MAX_FIELD + 1);
    assert!(!long.valid());
    assert!(
        !Outcome::Relayed {
            info: none,
            proof: proof()
        }
        .valid()
    );
    assert!(Outcome::Kept { proof: proof() }.valid());
    let mut unsigned = proof();
    unsigned.signature.clear();
    assert!(!Outcome::Kept { proof: unsigned }.valid());
}

#[test]
fn labels_are_bounded_and_left_out_of_carried_answers() {
    use crate::relay_label::RelayLabel;
    let mut labelled = info();
    labelled.labels.insert(
        "https://relay.example.org".into(),
        RelayLabel {
            name: Some("Home".into()),
            ..RelayLabel::default()
        },
    );
    assert!(labelled.valid());
    let json = serde_json::to_value(&labelled).unwrap();
    assert_eq!(json["labels"]["https://relay.example.org"]["name"], "Home");
    // Unsigned, so a carried answer sends none.
    assert_eq!(labelled.clone().without_labels(), info());
    assert!(
        serde_json::to_value(info())
            .unwrap()
            .get("labels")
            .is_none()
    );
    let mut broken = labelled;
    broken.labels.insert(
        "https://other.example.org".into(),
        RelayLabel {
            name: Some("line\nbreak".into()),
            ..RelayLabel::default()
        },
    );
    assert!(!broken.valid());
}

#[test]
fn host_relay_messages_keep_their_wire_names() {
    assert_eq!(
        serde_json::to_value(Request::HostRelays).unwrap(),
        json!({"request": "host_relays"})
    );
    assert_eq!(
        serde_json::to_value(Response::Relayed(Outcome::Relayed {
            info: info(),
            proof: proof()
        }))
        .unwrap(),
        json!({
            "response": "relayed",
            "outcome": "relayed",
            "identity": "identity",
            "relays": ["https://relay.example.org"],
            "route": "route",
            "proof": {"certificate": "CERT", "key": "KEY", "signature": "SIG"},
        })
    );
    assert_eq!(
        serde_json::to_value(State::Done(Outcome::Failed {
            reason: "no".into()
        }))
        .unwrap(),
        json!({"state": "done", "outcome": "failed", "reason": "no"})
    );
    assert_eq!(
        serde_json::to_value(State::Pending).unwrap(),
        json!({"state": "pending"})
    );
    // A job for a phone paired with that computer directly names no host.
    let job = Job {
        device: DeviceId("near".into()),
        host: None,
        request: request(),
    };
    let value = serde_json::to_value(&job).unwrap();
    assert!(value.get("host").is_none());
    assert_eq!(serde_json::from_value::<Job>(value).unwrap(), job);
    let relayed = Request::HostRelayed {
        device: DeviceId("near".into()),
        relay: Box::new(request()),
        outcome: Box::new(Outcome::Elsewhere),
    };
    let value = serde_json::to_value(&relayed).unwrap();
    assert_eq!(value["outcome"], json!({"outcome": "elsewhere"}));
    assert_eq!(serde_json::from_value::<Request>(value).unwrap(), relayed);
}

mod proof {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use p256::ecdsa::{Signature, SigningKey, signature::Signer};
    use sha2::{Digest, Sha256};

    fn encode(bytes: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn ca() -> SigningKey {
        SigningKey::from_slice(&[5; 32]).unwrap()
    }

    fn point(key: &SigningKey) -> Vec<u8> {
        key.verifying_key().to_sec1_point(false).as_bytes().to_vec()
    }

    /// Bytes holding the CA's key, as its certificate does.
    fn certificate() -> Vec<u8> {
        [b"head".as_slice(), &point(&ca()), b"tail"].concat()
    }

    fn request() -> host_relay::Request {
        let fingerprint: [u8; 32] = Sha256::digest(certificate()).into();
        host_relay::Request {
            fingerprint: fingerprint.iter().map(|b| format!("{b:02x}")).collect(),
            ..super::request()
        }
    }

    fn sign(request: &host_relay::Request, mut outcome: Outcome, with: &SigningKey) -> Outcome {
        let signature: Signature = with.sign(&outcome.message(request).unwrap());
        if let Outcome::Relayed { proof, .. } | Outcome::Kept { proof } = &mut outcome {
            proof.signature = encode(&signature.to_bytes());
        }
        outcome
    }

    fn relayed(certificate: &[u8], key: &SigningKey) -> Outcome {
        Outcome::Relayed {
            info: info(),
            proof: Proof {
                certificate: encode(certificate),
                key: encode(&point(key)),
                signature: String::new(),
            },
        }
    }

    #[test]
    fn an_answer_signed_by_the_pinned_ca_is_proven() {
        let request = request();
        assert!(sign(&request, relayed(&certificate(), &ca()), &ca()).proven(&request));
        let kept = Outcome::Kept {
            proof: Proof {
                certificate: encode(&certificate()),
                key: encode(&point(&ca())),
                signature: String::new(),
            },
        };
        assert!(sign(&request, kept, &ca()).proven(&request));
    }

    #[test]
    fn an_answer_from_any_other_host_is_not() {
        let request = request();
        let impostor = SigningKey::from_slice(&[6; 32]).unwrap();
        // Its own key, which the pinned certificate does not hold.
        assert!(!sign(&request, relayed(&certificate(), &impostor), &impostor).proven(&request));
        // Its own certificate, which does not hash to the fingerprint.
        let own = [b"head".as_slice(), &point(&impostor)].concat();
        assert!(!sign(&request, relayed(&own, &impostor), &impostor).proven(&request));
        // The real certificate and key, without the CA's signature.
        assert!(!sign(&request, relayed(&certificate(), &ca()), &impostor).proven(&request));
        // Relays changed after signing.
        let mut changed = sign(&request, relayed(&certificate(), &ca()), &ca());
        if let Outcome::Relayed { info, .. } = &mut changed {
            info.relays = vec!["https://elsewhere.example.org".into()];
        }
        assert!(!changed.proven(&request));
        // A signature for one request does not answer another.
        let answer = sign(&request, relayed(&certificate(), &ca()), &ca());
        let mut other = request.clone();
        other.signed_ms += 1;
        assert!(!answer.proven(&other));
        // Claims of nothing prove nothing.
        assert!(!Outcome::Elsewhere.proven(&request));
    }
}

/// The same bytes `HostRelayTests.swift` expects the phone to check.
#[test]
fn the_answer_message_is_the_request_then_what_it_claims() {
    let request = request();
    let signed = request.message().unwrap();
    let kept = Outcome::Kept { proof: proof() };
    let mut expected = ANSWER_PREFIX.to_vec();
    expected.extend(&signed);
    expected.push(2);
    assert_eq!(kept.message(&request).unwrap(), expected);
    let relayed = Outcome::Relayed {
        info: Info {
            identity: "ID".into(),
            labels: Default::default(),
            relays: vec!["https://a".into(), "https://b".into()],
            route: "RT".into(),
            rtt_ms: Default::default(),
        },
        proof: proof(),
    };
    let mut expected = ANSWER_PREFIX.to_vec();
    expected.extend(&signed);
    expected.extend([1, 0, 2]);
    expected.extend(b"ID");
    expected.extend([0, 2]);
    expected.extend(b"RT");
    expected.extend([0, 2, 0, 9]);
    expected.extend(b"https://a");
    expected.extend([0, 9]);
    expected.extend(b"https://b");
    assert_eq!(relayed.message(&request).unwrap(), expected);
    assert!(Outcome::Elsewhere.message(&request).is_none());
}
