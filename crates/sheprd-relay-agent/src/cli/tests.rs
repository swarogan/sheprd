#![allow(clippy::unwrap_used)]

use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::SigningKey;

fn parse_words(words: &[&str]) -> Result<Invocation, Error> {
    parse(words.iter().map(OsString::from))
}

fn dial() -> String {
    let key = SigningKey::from_slice(&[3; 32]).unwrap();
    URL_SAFE_NO_PAD.encode(sheprd_relay::client::public(&key))
}

#[test]
fn run_takes_relays_each_with_its_own_token() {
    let invocation = parse_words(&[
        "run",
        "--relay",
        "relay.example.org",
        "--token-file",
        "/etc/token",
        "--relay",
        "https://other.example.org",
        "--forward",
        "2222",
        "--state-dir",
        "/tmp/agent",
    ])
    .unwrap();
    assert_eq!(invocation.state_dir, Some(PathBuf::from("/tmp/agent")));
    let Command::Run(run) = invocation.command else {
        panic!("not run");
    };
    assert_eq!(run.forward, 2222);
    assert_eq!(run.relays.len(), 2);
    assert_eq!(run.relays[0].token_file, Some(PathBuf::from("/etc/token")));
    assert_eq!(run.relays[1].token_file, None);
}

#[test]
fn run_forwards_to_port_22_by_default() {
    let Command::Run(run) = parse_words(&["run", "--relay", "relay.example.org"])
        .unwrap()
        .command
    else {
        panic!("not run");
    };
    assert_eq!(run.forward, 22);
}

#[test]
fn run_needs_a_relay() {
    assert_eq!(parse_words(&["run"]), Err(Error::NoRelay));
}

#[test]
fn a_token_file_needs_a_relay_before_it() {
    assert_eq!(
        parse_words(&["run", "--token-file", "/t", "--relay", "relay.example.org"]),
        Err(Error::TokenWithoutRelay)
    );
}

#[test]
fn a_plain_http_relay_on_a_public_host_is_refused() {
    assert!(matches!(
        parse_words(&["run", "--relay", "http://relay.example.org"]),
        Err(Error::Endpoint(_))
    ));
}

#[test]
fn a_bad_port_is_refused() {
    assert!(matches!(
        parse_words(&["run", "--relay", "relay.example.org", "--forward", "99999"]),
        Err(Error::Port(_))
    ));
}

#[test]
fn authorize_takes_both_keys_and_a_label() {
    let tunnel = URL_SAFE_NO_PAD.encode([5; 32]);
    let invocation = parse_words(&["authorize", &dial(), &tunnel, "studio", "mac"]).unwrap();
    let Command::Authorize(entry) = invocation.command else {
        panic!("not authorize");
    };
    assert_eq!(entry.tunnel, [5; 32]);
    assert_eq!(entry.label, "studio mac");
}

#[test]
fn authorize_names_the_key_that_is_wrong() {
    let tunnel = URL_SAFE_NO_PAD.encode([5; 32]);
    assert!(matches!(
        parse_words(&["authorize", "nope", &tunnel]),
        Err(Error::DialKey(_))
    ));
    assert!(matches!(
        parse_words(&["authorize", &dial(), "nope"]),
        Err(Error::TunnelKey(_))
    ));
}

#[test]
fn revoke_takes_a_dial_key() {
    let invocation = parse_words(&["revoke", &dial()]).unwrap();
    assert!(matches!(invocation.command, Command::Revoke(_)));
    assert_eq!(
        parse_words(&["revoke"]),
        Err(Error::MissingValue("DIAL_KEY"))
    );
}

#[test]
fn status_takes_json() {
    let invocation = parse_words(&["status", "--json"]).unwrap();
    assert_eq!(invocation.command, Command::Status { json: true });
}

#[test]
fn another_commands_flags_are_refused() {
    assert_eq!(
        parse_words(&["status", "--relay", "relay.example.org"]),
        Err(Error::Unexpected("--relay".to_owned()))
    );
    assert_eq!(
        parse_words(&["run", "--relay", "relay.example.org", "--json"]),
        Err(Error::Unexpected("--json".to_owned()))
    );
    assert_eq!(
        parse_words(&["status", "extra"]),
        Err(Error::Unexpected("extra".to_owned()))
    );
}

#[test]
fn help_and_version_win_over_everything() {
    assert_eq!(
        parse_words(&["run", "--help"]).unwrap().command,
        Command::Help
    );
    assert_eq!(parse_words(&["-V"]).unwrap().command, Command::Version);
    assert_eq!(parse_words(&[]), Err(Error::NoCommand));
}

#[cfg(unix)]
#[test]
fn a_state_dir_need_not_be_utf8() {
    use std::os::unix::ffi::OsStringExt;
    let dir = OsString::from_vec(vec![b'/', 0xff]);
    let invocation = parse(["status".into(), "--state-dir".into(), dir.clone()]).unwrap();
    assert_eq!(invocation.state_dir, Some(PathBuf::from(dir)));
}
