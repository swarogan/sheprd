#![allow(clippy::unwrap_used)]

use super::*;

const ID: &str = "0123456789abcdef";

#[test]
fn host_names_round_trip_and_nothing_else_is_one() {
    let host = host_name(ID);
    assert_eq!(host, "r-0123456789abcdef.sheprd.invalid");
    assert_eq!(host_id(&host), Some(ID));
    for host in [
        "r-0123456789ABCDEF.sheprd.invalid",
        "r-0123456789abcde.sheprd.invalid",
        "x-0123456789abcdef.sheprd.invalid",
        "r-0123456789abcdef.sheprd.example",
        "r-0123456789abcdefsheprd.invalid",
        "example.org",
    ] {
        assert_eq!(host_id(host), None, "{host}");
    }
}

#[test]
fn targets_with_users_schemes_and_ports_are_recognised() {
    let host = host_name(ID);
    for target in [
        host.clone(),
        format!("me@{host}"),
        format!("ssh://me@{host}:22"),
        format!("{host}:2222"),
    ] {
        assert!(is_relay_target(&target), "{target}");
    }
    assert!(!is_relay_target("me@example.org"));
}

fn proxy(state_dir: &str) -> RelayProxy {
    RelayProxy {
        program: PathBuf::from("/Applications/Sheprd.app/Contents/MacOS/sheprd-remote-control"),
        state_dir: PathBuf::from(state_dir),
    }
}

// Windows paths have backslashes, which the proxy refuses: adding a nearby
// Sheprd is not offered there.
#[cfg(unix)]
#[test]
fn a_relay_machine_gets_the_proxy_identity_and_its_own_known_hosts() {
    let options =
        proxy_options(&proxy("/Users/me/Library/Application Support/100%h/remote")).unwrap();
    let values: Vec<&str> = options
        .iter()
        .skip(1)
        .step_by(2)
        .map(String::as_str)
        .collect();
    assert!(options.iter().step_by(2).all(|flag| flag == "-o"));
    assert_eq!(
        values[0],
        "ProxyCommand=\"/Applications/Sheprd.app/Contents/MacOS/sheprd-remote-control\" relay-connect --state-dir \"/Users/me/Library/Application Support/100%%h/remote\" %h"
    );
    assert_eq!(
        values[1],
        "IdentityFile=\"/Users/me/Library/Application Support/100%%h/remote/desktop/id_ed25519\""
    );
    assert!(values.contains(&"IdentitiesOnly=yes"));
    assert!(values.contains(&"StrictHostKeyChecking=yes"));
    assert!(values.contains(
        // OpenSSH expands `%h` to the machine's host name: a file each.
        &"UserKnownHostsFile=\"/Users/me/Library/Application Support/100%%h/remote/desktop/known_hosts.d/%h\""
    ));
}

#[test]
fn paths_the_shell_would_expand_are_refused() {
    for state_dir in ["/a/$HOME", "/a/`x`", "/a/\"b", "relative", "/a\\b"] {
        assert_eq!(proxy_options(&proxy(state_dir)), None, "{state_dir}");
    }
}

#[test]
fn other_targets_get_no_options() {
    assert!(options("me@example.org").is_empty());
}
