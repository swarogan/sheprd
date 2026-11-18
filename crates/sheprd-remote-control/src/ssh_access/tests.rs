use super::*;

/// A real `ssh-ed25519` key's base64 (32 bytes of 0x11).
fn blob() -> String {
    let bytes = [
        &11u32.to_be_bytes()[..],
        b"ssh-ed25519",
        &32u32.to_be_bytes(),
        &[0x11; 32],
    ]
    .concat();
    STANDARD.encode(bytes)
}

fn key() -> SshKey {
    SshKey::parse(&format!("ssh-ed25519 {} someone@laptop", blob())).unwrap()
}

#[test]
fn a_key_drops_its_comment() {
    assert_eq!(key().as_str(), format!("ssh-ed25519 {}", blob()));
}

#[test]
fn only_ed25519_keys_are_accepted() {
    for text in [
        String::new(),
        "ssh-ed25519".to_owned(),
        format!("ssh-rsa {}", blob()),
        "ssh-ed25519 not-base64!".to_owned(),
        format!("ssh-ed25519 {}", STANDARD.encode([0; 51])),
        format!("ssh-ed25519 {}", STANDARD.encode([0; 10])),
        format!("ssh-ed25519 {} {}", blob(), "x".repeat(MAX_KEY)),
    ] {
        assert!(
            matches!(SshKey::parse(&text), Err(Error::InvalidKey)),
            "{text}"
        );
    }
}

#[test]
fn allowing_adds_one_restricted_line_and_keeps_the_users() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".ssh").join("authorized_keys");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "ssh-ed25519 AAAA mine\n").unwrap();
    let keys = AuthorizedKeys::at(path.clone());
    let device = DeviceId("dev1".to_owned());

    keys.allow(&device, &key()).unwrap();
    keys.allow(&device, &key()).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "ssh-ed25519 AAAA mine");
    assert_eq!(
        lines[1],
        format!(
            r#"from="127.0.0.1,::1",no-agent-forwarding,no-X11-forwarding ssh-ed25519 {} sheprd-desktop:dev1"#,
            blob()
        )
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn removing_takes_only_that_devices_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("authorized_keys");
    let keys = AuthorizedKeys::at(path.clone());
    let one = DeviceId("one".to_owned());
    let two = DeviceId("two".to_owned());
    keys.allow(&one, &key()).unwrap();
    keys.allow(&two, &key()).unwrap();
    // A user's line that merely mentions the id stays.
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("ssh-ed25519 AAAA sheprd-desktop:one-copy\n");
    fs::write(&path, text).unwrap();

    assert!(keys.remove(&one).unwrap());
    assert!(!keys.remove(&one).unwrap());

    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("sheprd-desktop:one\n"));
    assert!(text.contains("sheprd-desktop:two\n"));
    assert!(text.contains("sheprd-desktop:one-copy\n"));
}

#[test]
fn removing_from_a_missing_file_creates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ssh").join("authorized_keys");
    let keys = AuthorizedKeys::at(path.clone());
    assert!(!keys.remove(&DeviceId("one".to_owned())).unwrap());
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn a_linked_file_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("managed");
    fs::write(&target, "").unwrap();
    let path = dir.path().join("authorized_keys");
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let keys = AuthorizedKeys::at(path);
    assert!(matches!(
        keys.allow(&DeviceId("one".to_owned()), &key()),
        Err(Error::Linked { .. })
    ));
}

#[test]
fn host_keys_drop_comments_and_refuse_junk() {
    assert_eq!(
        host_key("ssh-ed25519 AAAA root@host\n").as_deref(),
        Some("ssh-ed25519 AAAA")
    );
    assert_eq!(
        host_key("ecdsa-sha2-nistp256 AAAA").as_deref(),
        Some("ecdsa-sha2-nistp256 AAAA")
    );
    assert_eq!(host_key("garbage AAAA"), None);
    assert_eq!(host_key("ssh-ed25519 !!"), None);
}

#[test]
fn user_names_are_what_ssh_takes_as_is() {
    assert!(valid_user("penso"));
    assert!(valid_user("first.last_2"));
    assert!(!valid_user(""));
    assert!(!valid_user("-oProxyCommand=x"));
    assert!(!valid_user("a b"));
    assert!(!valid_user("a@b"));
}

#[test]
fn a_sweep_removes_only_lines_of_devices_that_are_gone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("authorized_keys");
    let keys = AuthorizedKeys::at(path.clone());
    keys.allow(&DeviceId("kept".to_owned()), &key()).unwrap();
    keys.allow(&DeviceId("gone".to_owned()), &key()).unwrap();
    let mut text = fs::read_to_string(&path).unwrap();
    // The user's own line, even one ending in the marker, is never Sheprd's.
    text.push_str("ssh-ed25519 AAAA sheprd-desktop:gone\n");
    fs::write(&path, text).unwrap();

    assert_eq!(keys.sweep(|id| id == "kept").unwrap(), 1);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("sheprd-desktop:kept\n"));
    assert!(text.contains("ssh-ed25519 AAAA sheprd-desktop:gone\n"));
    assert_eq!(text.lines().count(), 2);
    assert_eq!(keys.sweep(|id| id == "kept").unwrap(), 0);
}
