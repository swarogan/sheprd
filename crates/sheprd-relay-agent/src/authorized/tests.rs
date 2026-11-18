#![allow(clippy::unwrap_used)]

use super::*;
use p256::ecdsa::SigningKey;

fn dial(seed: u8) -> String {
    let key = SigningKey::from_slice(&[seed; 32]).unwrap();
    URL_SAFE_NO_PAD.encode(sheprd_relay::client::public(&key))
}

fn tunnel(seed: u8) -> String {
    URL_SAFE_NO_PAD.encode([seed; 32])
}

fn entry(seed: u8) -> Entry {
    Entry::parse(&dial(seed), &tunnel(seed), "desk").unwrap()
}

#[test]
fn a_saved_file_reads_back_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let mut authorized = Authorized::default();
    authorized.insert(entry(1)).unwrap();
    authorized
        .insert(Entry::parse(&dial(2), &tunnel(2), "  studio\tmac  ").unwrap())
        .unwrap();
    authorized.save(dir.path()).unwrap();
    let loaded = Authorized::load(dir.path()).unwrap();
    assert_eq!(loaded, authorized);
    assert_eq!(loaded.entries()[1].label, "studio mac");
}

#[cfg(unix)]
#[test]
fn the_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    Authorized::default().save(dir.path()).unwrap();
    let mode = std::fs::metadata(dir.path().join(FILE))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn a_missing_file_authorizes_no_one() {
    let dir = tempfile::tempdir().unwrap();
    let authorized = Authorized::load(dir.path()).unwrap();
    assert!(authorized.dial_keys().is_empty());
    assert!(!authorized.admits(&[1; 32]));
}

#[test]
fn comments_and_blank_lines_are_ignored() {
    let text = format!("# a comment\n\n  {} {}  \n", dial(3), tunnel(3));
    let Ok(authorized) = Authorized::parse(&text) else {
        panic!("did not parse");
    };
    assert_eq!(authorized.entries().len(), 1);
    assert_eq!(authorized.entries()[0].label, "");
}

#[test]
fn a_bad_line_names_its_number() {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("{} {}\nnot-a-key {}\n", dial(1), tunnel(1), tunnel(2));
    std::fs::write(dir.path().join(FILE), text).unwrap();
    assert!(matches!(
        Authorized::load(dir.path()),
        Err(Error::AuthorizedLine { line: 2, .. })
    ));
}

#[test]
fn a_dial_key_that_is_no_point_is_refused() {
    assert!(dial_key(&dial(9)).is_some());
    assert!(dial_key(&URL_SAFE_NO_PAD.encode([0; KEY_LEN])).is_none());
    assert!(dial_key(&URL_SAFE_NO_PAD.encode([0x04; KEY_LEN])).is_none());
    assert!(dial_key("short").is_none());
}

#[test]
fn a_short_tunnel_key_is_refused() {
    assert!(Entry::parse(&dial(1), &URL_SAFE_NO_PAD.encode([1; 31]), "").is_none());
}

#[test]
fn authorizing_again_replaces_the_desktop() {
    let mut authorized = Authorized::default();
    authorized.insert(entry(1)).unwrap();
    // The same dial key with a new tunnel key: the desktop made new keys.
    let renewed = Entry::parse(&dial(1), &tunnel(7), "desk").unwrap();
    authorized.insert(renewed.clone()).unwrap();
    assert_eq!(authorized.entries(), [renewed]);
    assert!(!authorized.admits(&[1; 32]));
    assert!(authorized.admits(&[7; 32]));
}

#[test]
fn removing_a_desktop_stops_admitting_it() {
    let mut authorized = Authorized::default();
    authorized.insert(entry(1)).unwrap();
    authorized.insert(entry(2)).unwrap();
    assert!(authorized.remove(&entry(1).dial));
    assert!(!authorized.remove(&entry(1).dial));
    assert!(!authorized.admits(&[1; 32]));
    assert_eq!(authorized.dial_keys(), [entry(2).dial]);
}

#[test]
fn no_more_desktops_than_a_relay_takes_dial_keys_for() {
    let mut authorized = Authorized::default();
    for seed in 1..=u8::try_from(MAX_ENTRIES).unwrap() {
        authorized.insert(entry(seed)).unwrap();
    }
    assert_eq!(authorized.insert(entry(200)), Err(Full));
}

#[test]
fn a_long_label_is_cut_on_a_character_boundary() {
    let label = "é".repeat(MAX_LABEL);
    let entry = Entry::parse(&dial(1), &tunnel(1), &label).unwrap();
    assert!(entry.label.len() <= MAX_LABEL);
    assert!(entry.label.chars().all(|c| c == 'é'));
}

#[test]
fn a_label_cannot_add_a_line() {
    let entry = Entry::parse(&dial(1), &tunnel(1), "desk\nAAAA BBBB").unwrap();
    assert_eq!(entry.label, "desk AAAA BBBB");
    let mut authorized = Authorized::default();
    authorized.insert(entry).unwrap();
    assert_eq!(authorized.render().lines().count(), 2);
}
