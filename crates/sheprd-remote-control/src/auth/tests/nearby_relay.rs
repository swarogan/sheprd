//! Nearby pairing for a computer that listens on relays: the sealed offer
//! names them, its token goes to the relays once for a pairing dial, and
//! the keys the phone claims with are stored when the desktop approves.

use super::nearby::{Phone, exchange, open};
use super::*;
use crate::{auth::devices::RelayKeys, relay::Info};

fn info() -> Info {
    Info {
        identity: "identity".into(),
        relays: vec!["https://relay.example.org".into()],
        route: "route".into(),
    }
}

#[test]
fn the_offer_names_the_relays_and_its_token_opens_one_pairing_dial() {
    let dir = tempfile::tempdir().unwrap();
    let (mut auth, _) = open(dir.path());
    auth.set_relay_offer(info());
    let phone = Phone::new(3, 4);
    let (started, revealed) = exchange(&mut auth, &phone, T0);
    let finished = phone.finish(&started, &revealed).unwrap();
    let relay = finished.relay.expect("the offer names no relay");
    assert_eq!(relay["relays"][0], "https://relay.example.org");
    assert_eq!(relay["route"], "route");
    let secret = auth
        .take_relay_pairing()
        .expect("no pairing for the relays");
    assert_eq!(encode(&secret), finished.token);
    assert!(auth.take_relay_pairing().is_none());
}

#[test]
fn a_computer_without_relays_offers_none() {
    let dir = tempfile::tempdir().unwrap();
    let (mut auth, _) = open(dir.path());
    let phone = Phone::new(3, 4);
    let (started, revealed) = exchange(&mut auth, &phone, T0);
    assert!(phone.finish(&started, &revealed).unwrap().relay.is_none());
    assert!(auth.take_relay_pairing().is_none());
}

#[test]
fn the_relay_keys_a_phone_claims_with_are_stored_when_approved() {
    let dir = tempfile::tempdir().unwrap();
    let (mut auth, offer) = open(dir.path());
    auth.set_relay_offer(info());
    let phone = Phone::new(3, 4);
    let (started, revealed) = exchange(&mut auth, &phone, T0);
    let finished = phone.finish(&started, &revealed).unwrap();
    let claim = auth
        .claim(
            "10.0.0.9".parse().unwrap(),
            &finished.token,
            "iPhone",
            &public_key(&signing_key(9)),
            T0,
        )
        .unwrap();
    let dial = SigningKey::from_slice(&[7; 32]).unwrap();
    let keys = RelayKeys {
        dial: encode(&sheprd_relay::client::public(&dial)),
        tunnel: encode(&[8; 32]),
    };
    assert!(keys.dial_key().is_some() && keys.tunnel_key().is_some());
    auth.claimed_relay(&claim.id, keys.clone());
    auth.answer(&offer.id, true, &claim.code, T0).unwrap();
    let record = auth.devices().records().last().unwrap();
    assert_eq!(record.relay.as_ref(), Some(&keys));
}
