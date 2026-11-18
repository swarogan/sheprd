use super::*;

fn offer(source: &str, relays: &[&str]) -> Offer {
    Offer {
        source: source.into(),
        relays: relays.iter().map(|relay| (*relay).to_owned()).collect(),
    }
}

#[test]
fn with_no_daemon_an_offer_waits_in_the_file_the_daemon_reads() {
    let dir = tempfile::tempdir().unwrap();
    let listened = share(dir.path(), offer("studio", &["relay.example.org"])).unwrap();
    assert_eq!(listened, ["https://relay.example.org"]);
    let saved = load(&file(dir.path()));
    assert_eq!(saved.endpoints().len(), 1);

    let listened = share(dir.path(), offer("studio", &[])).unwrap();
    assert!(listened.is_empty());
    assert!(load(&file(dir.path())).endpoints().is_empty());
}

#[test]
fn a_refused_offer_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    share(dir.path(), offer("studio", &["relay.example.org"])).unwrap();
    let refused = share(dir.path(), offer("studio", &["ftp://relay.example.org"]));
    assert!(matches!(refused, Err(Error::Offer(_))));
    assert_eq!(load(&file(dir.path())).endpoints().len(), 1);
}

#[test]
fn a_damaged_file_reads_as_nothing_shared() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{not json").unwrap();
    assert!(load(&path).endpoints().is_empty());
}

#[cfg(unix)]
#[test]
fn the_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    share(dir.path(), offer("studio", &["relay.example.org"])).unwrap();
    let mode = fs::metadata(file(dir.path())).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn with_no_daemon_the_relays_listened_on_are_the_shared_ones() {
    let dir = tempfile::tempdir().unwrap();
    assert!(listened(dir.path()).unwrap().is_empty());
    share(dir.path(), offer("studio", &["relay.example.org"])).unwrap();
    assert_eq!(listened(dir.path()).unwrap(), ["https://relay.example.org"]);
}
