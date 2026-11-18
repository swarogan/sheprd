use super::*;

fn token(text: &str) -> SecretString {
    SecretString::from(text.to_owned())
}

#[test]
fn a_saved_token_is_only_for_the_server_that_issued_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t3-token");
    save(&path, "env-a", &token("tok")).unwrap();
    assert_eq!(load(&path, "env-a").unwrap().expose_secret(), "tok");
    assert!(matches!(
        load(&path, "env-b"),
        Err(Error::OtherServer { .. })
    ));
    // Pairing again replaces it.
    save(&path, "env-b", &token("tok2")).unwrap();
    assert_eq!(load(&path, "env-b").unwrap().expose_secret(), "tok2");
    // Nothing staged is left behind.
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn a_saved_token_is_private_whatever_was_at_the_path() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t3-token");
    fs::write(&path, "old").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    save(&path, "env", &token("tok")).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // A link at the path is replaced, never written through.
    let target = dir.path().join("elsewhere");
    fs::write(&target, "untouched").unwrap();
    let link = dir.path().join("linked");
    symlink(&target, &link).unwrap();
    save(&link, "env", &token("tok")).unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
    assert!(
        !fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn a_missing_foreign_or_oversized_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("none");
    assert!(matches!(load(&missing, "env"), Err(Error::Read { .. })));
    let plain = dir.path().join("plain");
    fs::write(&plain, "just-a-token").unwrap();
    assert!(matches!(load(&plain, "env"), Err(Error::Invalid { .. })));
    let big = dir.path().join("big");
    fs::write(&big, " ".repeat(LIMIT as usize + 1)).unwrap();
    assert!(matches!(load(&big, "env"), Err(Error::Invalid { .. })));
}
