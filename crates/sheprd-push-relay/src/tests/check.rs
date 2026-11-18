//! `--check`: the secrets the droplet's start.sh swaps in only when the
//! relay could start with them.

use crate::{Error, Options, serve, serve::Secrets};
use p256::{
    SecretKey,
    pkcs8::{EncodePrivateKey as _, LineEnding},
};
use std::{ffi::OsString, fs, path::Path};

fn write_private(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn pem(scalar: u8) -> String {
    SecretKey::from_slice(&[scalar; 32])
        .unwrap()
        .to_pkcs8_pem(LineEnding::LF)
        .unwrap()
        .to_string()
}

/// A directory in the layout deploy/push-relay/make-secrets.sh writes.
fn secrets(dir: &Path) {
    write_private(
        &dir.join("apns/config.json"),
        r#"{"team_id":"ABCDE12345","key_id":"XYZ9876543","topic":"so.pen.herdr.remote","sandbox":true}"#,
    );
    write_private(&dir.join("apns/AuthKey.p8"), &pem(7));
    write_private(&dir.join("relay-key.pem"), &pem(8));
    write_private(&dir.join("credential-secret"), &"ab".repeat(32));
}

fn check(dir: &Path) -> crate::Result<()> {
    let options =
        Options::parse(["--secrets", dir.to_str().unwrap(), "--check"].map(OsString::from))
            .unwrap()
            .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(serve(options))
}

#[test]
fn good_secrets_pass_without_opening_a_store_or_socket() {
    let dir = tempfile::tempdir().unwrap();
    secrets(dir.path());
    check(dir.path()).unwrap();
    let mut names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["apns", "credential-secret", "relay-key.pem"]);
}

#[test]
fn an_empty_or_malformed_key_is_refused() {
    for file in ["apns/AuthKey.p8", "relay-key.pem", "relay-key-previous.pem"] {
        for contents in ["", "not a key"] {
            let dir = tempfile::tempdir().unwrap();
            secrets(dir.path());
            write_private(&dir.path().join(file), contents);
            assert!(
                matches!(check(dir.path()), Err(Error::Apns(_))),
                "{file}: {contents:?}"
            );
        }
    }
}

#[test]
fn a_short_credential_secret_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    secrets(dir.path());
    write_private(&dir.path().join("credential-secret"), "abcd");
    assert!(matches!(check(dir.path()), Err(Error::SecretSize { .. })));
}

#[test]
fn missing_apns_config_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    secrets(dir.path());
    fs::remove_file(dir.path().join("apns/config.json")).unwrap();
    assert!(matches!(
        Secrets::load(dir.path()),
        Err(Error::NoApnsConfig { .. })
    ));
}

#[test]
fn check_is_parsed() {
    let options = Options::parse(["--secrets", "/s", "--check"].map(OsString::from))
        .unwrap()
        .unwrap();
    assert!(options.check);
    let options = Options::parse(["--secrets", "/s"].map(OsString::from))
        .unwrap()
        .unwrap();
    assert!(!options.check);
}
