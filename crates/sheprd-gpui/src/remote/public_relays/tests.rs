#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use sheprd_relay::{Endpoint, directory::Relay};

/// A list the site workflow signed, from the relay crate's own fixtures.
const SIGNED: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sheprd-relay/src/tests/fixtures/relays-signed.json"
));
const SIGNED_SIG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../sheprd-relay/src/tests/fixtures/relays-signed.json.sig"
));
/// When the fixture was signed, in Unix seconds.
const SIGNED_AT: u64 = 1_794_506_300;

#[test]
fn a_signed_list_is_offered_with_who_vouched_for_it() {
    let signatures = Signatures {
        ed25519: Some(SIGNED_SIG),
        ..Signatures::default()
    };
    let listed = listed(SIGNED, signatures, SIGNED_AT).unwrap();
    assert_eq!(listed.signer, Signer::Actions);
    assert!(listed.relays.is_empty());
}

#[test]
fn a_list_nobody_signed_or_that_expired_offers_nothing() {
    let unsigned = listed(SIGNED, Signatures::default(), SIGNED_AT);
    assert!(matches!(
        unsigned,
        Err(Error::Refused(directory::Error::Unsigned))
    ));
    let signatures = Signatures {
        ed25519: Some(SIGNED_SIG),
        ..Signatures::default()
    };
    let expired = listed(SIGNED, signatures, 4_102_444_800);
    assert!(matches!(
        expired,
        Err(Error::Refused(directory::Error::Expired { .. }))
    ));
}

#[test]
fn each_relay_is_offered_by_the_address_the_daemon_names_it_by() {
    let listed = Listed::from(signed::Signed {
        directory: Directory {
            expires: Some(2),
            relays: vec![Relay {
                endpoint: Endpoint::parse("https://fra1.relay.sheprd.org/").unwrap(),
                region: Some("fra1".into()),
                name: Some("Frankfurt".into()),
                quic_pin: None,
            }],
            serial: Some(1),
        },
        expires: 2,
        serial: 1,
        signer: Signer::Maintainer,
    });
    assert!(!listed.expired(1) && listed.expired(2));
    assert_eq!(
        listed.relays,
        [Public {
            address: "https://fra1.relay.sheprd.org".into(),
            name: Some("Frankfurt".into()),
            region: Some("fra1".into()),
        }]
    );
}

fn failed() -> Result<Option<Vec<u8>>, Error> {
    Err(Error::Download(ureq::Error::ConnectionFailed))
}

#[test]
fn one_signature_failing_to_download_leaves_the_other_to_vouch() {
    let signature = || Ok(Some(SIGNED_SIG.to_vec()));
    let listed = settle(SIGNED, failed(), signature(), SIGNED_AT).unwrap();
    assert_eq!(listed.signer, Signer::Actions);
    // Neither arrived, or the one that did vouches for nothing: the failed
    // download is why, not a list nobody signed.
    for ed25519 in [failed(), Ok(None), Ok(Some(b"nonsense".to_vec()))] {
        let refused = settle(SIGNED, failed(), ed25519, SIGNED_AT);
        assert!(matches!(refused, Err(Error::Download(_))));
    }
    // Every download worked: the list itself is what is refused.
    let unsigned = settle(SIGNED, Ok(None), Ok(None), SIGNED_AT);
    assert!(matches!(
        unsigned,
        Err(Error::Refused(directory::Error::Unsigned))
    ));
    let expired = settle(SIGNED, failed(), signature(), 4_102_444_800);
    assert!(matches!(
        expired,
        Err(Error::Refused(directory::Error::Expired { .. }))
    ));
}
