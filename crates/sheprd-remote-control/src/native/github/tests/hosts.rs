use super::*;
use crate::native::github::hosts::{GhHost, Recognized, file_in, parse, read};
use std::{ffi::OsString, path::PathBuf};

const HOSTS_YML: &str = "\
github.com:
    users:
        octocat:
            oauth_token: gho_never_read
        other:
    git_protocol: ssh
    oauth_token: gho_never_read
    user: octocat
\"ghe.example.com\":
  user: 'enterprise-me'
  git_protocol: https
broken host:
    user: nobody
nouser.example.com:
    git_protocol: ssh
";

#[test]
fn gh_hosts_name_each_host_and_its_user_and_skip_tokens() {
    let hosts = parse(HOSTS_YML);
    assert_eq!(
        hosts,
        [
            GhHost {
                host: "github.com".into(),
                login: "octocat".into(),
            },
            GhHost {
                host: "ghe.example.com".into(),
                login: "enterprise-me".into(),
            },
        ]
    );
    assert!(!format!("{hosts:?}").contains("gho_"));
}

#[test]
fn a_missing_or_huge_hosts_file_lends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read(&dir.path().join("hosts.yml")).is_empty());
    let huge = dir.path().join("huge.yml");
    std::fs::write(&huge, format!("{HOSTS_YML}{}", "#\n".repeat(40_000))).unwrap();
    assert!(read(&huge).is_empty());
    let file = dir.path().join("hosts.yml");
    std::fs::write(&file, HOSTS_YML).unwrap();
    assert_eq!(read(&file).len(), 2);
}

#[test]
fn github_hosts_are_known_never_guessed() {
    let recognized = Recognized {
        configured: vec!["git.corp.example".into()],
        gh: parse(HOSTS_YML),
    };
    for host in [
        "github.com",
        "acme.ghe.com",
        "ghe.example.com",
        "git.corp.example",
    ] {
        assert!(recognized.is_github(host), "{host}");
    }
    for host in [
        "gitlab.com",
        "github.example.com",
        "ghe.com",
        "a.b.ghe.com",
        "evilgithub.com",
    ] {
        assert!(!recognized.is_github(host), "{host}");
    }
    assert_eq!(
        recognized.gh_login("ghe.example.com"),
        Some("enterprise-me")
    );
    assert_eq!(recognized.gh_login("git.corp.example"), None);
}

#[test]
fn configured_hosts_that_are_not_host_names_are_dropped() {
    let lending = Lending::new(
        vec![
            "GHE.Example.com".into(),
            "https://bad.example.com".into(),
            String::new(),
        ],
        |_| Err(sheprd_github::Error::Token),
    );
    assert_eq!(lending.recognized().configured, ["ghe.example.com"]);
}

/// A lookup that reads `login` first and `process` after, as
/// `login_env::var` does.
fn lookup<'a>(
    login: &'a [(&str, &str)],
    process: &'a [(&str, &str)],
) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        login
            .iter()
            .chain(process)
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn the_login_shells_gh_config_dir_wins_over_the_daemons() {
    let found = file_in(lookup(
        &[("GH_CONFIG_DIR", "/login/gh")],
        &[("GH_CONFIG_DIR", "/daemon/gh"), ("HOME", "/home/me")],
    ));
    assert_eq!(found, Some(PathBuf::from("/login/gh/hosts.yml")));
    let found = file_in(lookup(
        &[("XDG_CONFIG_HOME", "/login/config")],
        &[("HOME", "/home/me")],
    ));
    assert_eq!(found, Some(PathBuf::from("/login/config/gh/hosts.yml")));
}

#[test]
fn hosts_file_falls_back_when_the_variables_are_unset_or_empty() {
    let found = file_in(lookup(
        &[("GH_CONFIG_DIR", ""), ("XDG_CONFIG_HOME", "")],
        &[("HOME", "/home/me")],
    ));
    assert_eq!(
        found,
        Some(
            PathBuf::from("/home/me")
                .join(".config")
                .join("gh")
                .join("hosts.yml")
        )
    );
    assert_eq!(file_in(lookup(&[], &[])), None);
}
