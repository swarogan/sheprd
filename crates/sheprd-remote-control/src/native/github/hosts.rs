//! Which hosts are GitHub hosts: github.com, GHE.com's `*.ghe.com`, the
//! hosts `gh` is signed in to on this computer, and those the user lists in
//! `[remote] github_hosts`. A host is never guessed from a URL's look.
//!
//! `gh`'s hosts come from its `hosts.yml`, found once through the login
//! shell's environment and read on a worker every minute, never while a
//! request waits. Only host names and logins are read from
//! it: a token `gh` stored there in plain text is skipped.

use crate::control::github::{GITHUB_COM, valid_host, valid_login};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// The most of `hosts.yml` read; a larger file is not `gh`'s.
const MAX_FILE: u64 = 64 * 1024;
/// The most hosts kept from it.
const MAX_HOSTS: usize = 32;

/// A host `gh` is signed in to, and the account it uses there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GhHost {
    pub(crate) host: String,
    pub(crate) login: String,
}

/// The hosts this computer counts as GitHub's.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Recognized {
    /// `[remote] github_hosts`.
    pub(crate) configured: Vec<String>,
    pub(crate) gh: Vec<GhHost>,
}

impl Recognized {
    pub(crate) fn is_github(&self, host: &str) -> bool {
        host == GITHUB_COM
            || (valid_host(host)
                && host
                    .strip_suffix(".ghe.com")
                    .is_some_and(|tenant| !tenant.is_empty() && !tenant.contains('.')))
            || self.configured.iter().any(|known| known == host)
            || self.gh.iter().any(|known| known.host == host)
    }

    /// The account `gh` uses on `host`, when it is signed in there.
    pub(crate) fn gh_login(&self, host: &str) -> Option<&str> {
        self.gh
            .iter()
            .find(|known| known.host == host)
            .map(|known| known.login.as_str())
    }
}

/// Where `gh` keeps its hosts, as `gh` itself looks: `GH_CONFIG_DIR`, then
/// `XDG_CONFIG_HOME/gh`, then `%AppData%\GitHub CLI` on Windows, then
/// `~/.config/gh`. Each is read as the `gh` that lends a token sees it: the
/// login shell's value, then this process's, as a daemon started by launchd
/// or systemd has none of the shell profile's. Blocking the first time:
/// call it on the blocking pool.
pub(crate) fn file() -> Option<PathBuf> {
    file_in(sheprd_client::login_env::var)
}

/// [`file`], with each variable read through `var`.
pub(crate) fn file_in(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let var = |name| var(name).filter(|value| !value.is_empty());
    let dir = var("GH_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| var("XDG_CONFIG_HOME").map(|dir| PathBuf::from(dir).join("gh")))
        .or_else(|| {
            cfg!(windows)
                .then(|| var("AppData"))
                .flatten()
                .map(|dir| PathBuf::from(dir).join("GitHub CLI"))
        })
        .or_else(|| var("HOME").map(|home| PathBuf::from(home).join(".config").join("gh")))?;
    Some(dir.join("hosts.yml"))
}

/// The hosts in the `hosts.yml` at `path`; none when it is missing or
/// unreadable. Blocking.
pub(crate) fn read(path: &Path) -> Vec<GhHost> {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut text = String::new();
    if file.take(MAX_FILE + 1).read_to_string(&mut text).is_err() || text.len() as u64 > MAX_FILE {
        return Vec::new();
    }
    parse(&text)
}

/// The hosts of `gh`'s `hosts.yml`: each top-level key, with the `user`
/// directly beneath it. A host without one is left out, as it lends no
/// account a phone could name.
pub(crate) fn parse(text: &str) -> Vec<GhHost> {
    let mut hosts: Vec<GhHost> = Vec::new();
    // The host being read, and the indent of its own keys.
    let mut current: Option<(String, Option<usize>)> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            current = trimmed
                .strip_suffix(':')
                .map(|key| unquote(key.trim()).to_ascii_lowercase())
                .filter(|host| valid_host(host))
                .map(|host| (host, None));
            continue;
        }
        let Some((host, level)) = &mut current else {
            continue;
        };
        if *level.get_or_insert(indent) != indent {
            continue;
        }
        let Some(login) = trimmed.strip_prefix("user:") else {
            continue;
        };
        let login = unquote(login.trim());
        if valid_login(login)
            && hosts.len() < MAX_HOSTS
            && !hosts.iter().any(|known| &known.host == host)
        {
            hosts.push(GhHost {
                host: host.clone(),
                login: login.to_owned(),
            });
        }
    }
    hosts
}

fn unquote(value: &str) -> &str {
    ['"', '\'']
        .into_iter()
        .find_map(|quote| {
            value
                .strip_prefix(quote)
                .and_then(|value| value.strip_suffix(quote))
        })
        .unwrap_or(value)
}
