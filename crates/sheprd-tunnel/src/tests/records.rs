//! Records: padding, order, tampering, rekeying, and closing.

use super::*;
use crate::{
    Error,
    record::{MAX_DATA, Opened, PAD, REKEY_AFTER},
};
use std::time::Instant;

#[test]
fn records_are_padded_to_buckets() {
    let (mut dialing, _) = session(&keys(1), &keys(2));
    let now = Instant::now();
    let short = dialing.seal(b"a", now).unwrap();
    let longer = dialing.seal(&[7; 200], now).unwrap();
    assert_eq!(short.len(), longer.len());
    assert_eq!(short.len(), PAD + 16);
    let big = dialing.seal(&[7; 300], now).unwrap();
    assert_eq!(big.len(), 2 * PAD + 16);
    assert!(dialing.seal(&vec![0; MAX_DATA + 1], now).is_err());
    assert!(dialing.seal(&vec![0; MAX_DATA], now).is_ok());
}

#[test]
fn a_replayed_reordered_or_changed_record_is_refused() {
    let (mut dialing, mut listening) = session(&keys(1), &keys(2));
    let now = Instant::now();
    let first = dialing.seal(b"one", now).unwrap();
    let second = dialing.seal(b"two", now).unwrap();
    assert!(matches!(listening.open(&second), Err(Error::Record(_))));
    let (mut dialing, mut listening) = session(&keys(1), &keys(2));
    let first_again = dialing.seal(b"one", now).unwrap();
    assert_eq!(
        listening.open(&first_again).unwrap(),
        Opened::Data(b"one".to_vec())
    );
    assert!(listening.open(&first_again).is_err());
    let (mut dialing, mut listening) = session(&keys(1), &keys(2));
    let mut changed = dialing.seal(b"one", now).unwrap();
    changed[5] ^= 1;
    assert!(listening.open(&changed).is_err());
    let _ = first;
}

#[test]
fn rekeying_keeps_both_sides_in_step() {
    let (mut dialing, mut listening) = session(&keys(1), &keys(2));
    let start = Instant::now();
    // By time: the record after ten minutes carries the flag.
    let before = dialing.seal(b"before", start).unwrap();
    let flagged = dialing.seal(b"flagged", start + REKEY_AFTER).unwrap();
    let after = dialing.seal(b"after", start + REKEY_AFTER).unwrap();
    for (record, data) in [(before, "before"), (flagged, "flagged"), (after, "after")] {
        assert_eq!(
            listening.open(&record).unwrap(),
            Opened::Data(data.as_bytes().to_vec())
        );
    }
    // A record sealed before the rekey no longer opens after it.
    let (mut dialing, mut listening) = session(&keys(1), &keys(2));
    let flagged = dialing.seal(b"x", start).unwrap();
    let _ = flagged;
    let late = dialing.seal(b"y", start + REKEY_AFTER).unwrap();
    assert!(listening.open(&late).is_err());
}

#[test]
fn closing_is_told_apart_from_data() {
    let (mut dialing, mut listening) = session(&keys(1), &keys(2));
    let close = dialing.seal_close().unwrap();
    assert_eq!(listening.open(&close).unwrap(), Opened::Closed);
}

#[test]
fn short_or_unknown_records_are_refused() {
    let (_, mut listening) = session(&keys(1), &keys(2));
    assert!(matches!(listening.open(&[0; 10]), Err(Error::Malformed)));
    assert!(listening.open(&[0; PAD + 16]).is_err());
}
