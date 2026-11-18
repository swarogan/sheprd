//! A machine's record is kept under the start of its route: the same
//! machine may update it, another whose route starts alike may not.

use super::*;
use crate::desktop::record::Record;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

fn route(last: u8) -> String {
    let mut bytes = [7; 32];
    bytes[31] = last;
    URL_SAFE_NO_PAD.encode(bytes)
}

fn record(route: String, identity: &str) -> Record {
    Record {
        computer: "Studio".into(),
        identity: identity.into(),
        relays: vec!["https://relay.example.org".into()],
        route,
    }
}

#[test]
fn the_same_machine_updates_its_record_and_another_is_refused() {
    let here = tempfile::tempdir().unwrap();
    let key = vec!["ssh-ed25519 AAAA".to_owned()];
    let first = record(route(1), "identity-one");
    let id = first.save(here.path(), &key).unwrap();

    // Paired again with new relays and host keys: fine.
    let again = Record {
        relays: vec!["https://other.example.org".into()],
        ..first.clone()
    };
    assert_eq!(
        again
            .save(here.path(), &["ssh-ed25519 BBBB".to_owned()])
            .unwrap(),
        id
    );

    // Another route with the same first eight bytes, or another identity.
    for impostor in [
        record(route(2), "identity-one"),
        record(route(1), "identity-two"),
    ] {
        assert_eq!(impostor.id().unwrap(), id);
        assert!(
            matches!(
                impostor.save(here.path(), &key),
                Err(Error::RecordConflict(_))
            ),
            "{impostor:?}"
        );
    }
    let kept = Record::load(here.path(), &id).unwrap();
    assert_eq!(kept, again);
    let known = std::fs::read_to_string(sheprd_client::relay_host::known_hosts(
        here.path(),
        &sheprd_client::relay_host::host_name(&id),
    ))
    .unwrap();
    assert!(known.contains("ssh-ed25519 BBBB") && !known.contains("ssh-ed25519 AAAA"));
}
