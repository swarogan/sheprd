use super::*;
use crate::remote::public_relays::Error;

#[test]
fn every_public_relays_variant_has_a_message() {
    assert_localized(&[
        Error::Download(ureq::Error::ConnectionFailed),
        Error::Missing,
        Error::Refused(sheprd_relay::directory::Error::Unsigned),
    ]);
}
