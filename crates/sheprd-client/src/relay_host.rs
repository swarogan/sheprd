//! SSH machines reached only through a relay: another computer's Sheprd
//! added over Bluetooth from another network (`docs/relay-plan.md`, Adding
//! a nearby Sheprd from another network).
//!
//! Such a machine is saved as `USER@r-ID.sheprd.invalid`. `.invalid` never
//! resolves (RFC 2606), so nothing but the relay can carry it: every `ssh`
//! this crate starts for such a target gets a `ProxyCommand` that runs
//! `sheprd-remote-control relay-connect`, the identity and known hosts
//! file pairing made, and strict host key checking. The user's
//! `~/.ssh/config` is never edited, and options given on the command line
//! come before it. Reading the proxy never blocks on I/O, so command
//! builders stay safe on the UI thread.

use std::{
    path::{Path, PathBuf},
    sync::{PoisonError, RwLock},
};

/// The reserved domain relay machines are named under.
pub const DOMAIN: &str = "sheprd.invalid";
/// Hex digits in a machine's id: the start of its route.
pub const ID_LEN: usize = 16;
/// The folder, in the remote control state directory, pairing keeps a
/// desktop's keys, known hosts, and machine records in.
pub const DIRECTORY: &str = "desktop";
/// The SSH key this computer logs in to relay machines with.
pub const IDENTITY: &str = "id_ed25519";
/// The folder of host keys pairing learned: one file per machine, named
/// for its host name, so two pairings finishing at once never write the
/// same file.
pub const KNOWN_HOSTS: &str = "known_hosts.d";

/// The program and state directory `relay-connect` runs with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayProxy {
    /// `sheprd-remote-control`, beside the app.
    pub program: PathBuf,
    /// The remote control state directory holding [`DIRECTORY`].
    pub state_dir: PathBuf,
}

static PROXY: RwLock<Option<RelayProxy>> = RwLock::new(None);

/// Lets `ssh` reach relay machines through `proxy`; `None` leaves them
/// unreachable, as `.invalid` never resolves.
pub fn set_relay_proxy(proxy: Option<RelayProxy>) {
    *PROXY.write().unwrap_or_else(PoisonError::into_inner) = proxy;
}

/// The host name of the machine `id`.
#[must_use]
pub fn host_name(id: &str) -> String {
    format!("r-{id}.{DOMAIN}")
}

/// The id in a relay machine's host name, `r-ID.sheprd.invalid`.
#[must_use]
pub fn host_id(host: &str) -> Option<&str> {
    let id = host
        .strip_suffix(DOMAIN)?
        .strip_suffix('.')?
        .strip_prefix("r-")?;
    let valid = id.len() == ID_LEN
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    valid.then_some(id)
}

/// The host part of an SSH target: `[ssh://][USER@]HOST[:PORT]`.
fn target_host(target: &str) -> &str {
    let authority = target.strip_prefix("ssh://").unwrap_or(target);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    host.split_once(':').map_or(host, |(host, _)| host)
}

/// Whether `target` names a relay machine.
#[must_use]
pub fn is_relay_target(target: &str) -> bool {
    host_id(target_host(target)).is_some()
}

/// The file holding the host keys of the machine named `host`.
#[must_use]
pub fn known_hosts(state_dir: &Path, host: &str) -> PathBuf {
    directory(state_dir).join(KNOWN_HOSTS).join(host)
}

/// The folder pairing keeps its files in, under `state_dir`.
#[must_use]
pub fn directory(state_dir: &Path) -> PathBuf {
    state_dir.join(DIRECTORY)
}

/// Options placed before every other for `target`, which they override:
/// none unless it names a relay machine and a proxy is set.
pub(crate) fn options(target: &str) -> Vec<String> {
    if !is_relay_target(target) {
        return Vec::new();
    }
    let proxy = PROXY.read().unwrap_or_else(PoisonError::into_inner);
    let Some(proxy) = proxy.as_ref() else {
        return Vec::new();
    };
    proxy_options(proxy).unwrap_or_default()
}

/// The options for `proxy`; `None` when a path cannot be written safely in
/// an SSH option.
fn proxy_options(proxy: &RelayProxy) -> Option<Vec<String>> {
    let directory = directory(&proxy.state_dir);
    let program = quoted(&proxy.program)?;
    let state_dir = quoted(&proxy.state_dir)?;
    let identity = quoted(&directory.join(IDENTITY))?;
    // `%h` is OpenSSH's token for the host name, added after quoting so
    // it is not escaped as the path's own `%` signs are.
    let folder = quoted(&directory.join(KNOWN_HOSTS))?;
    let known_hosts = format!("{}/%h\"", folder.strip_suffix('"')?);
    Some(
        [
            format!("ProxyCommand={program} relay-connect --state-dir {state_dir} %h"),
            format!("IdentityFile={identity}"),
            "IdentitiesOnly=yes".to_owned(),
            format!("UserKnownHostsFile={known_hosts}"),
            "GlobalKnownHostsFile=/dev/null".to_owned(),
            "StrictHostKeyChecking=yes".to_owned(),
            "CheckHostIP=no".to_owned(),
            "ProxyJump=none".to_owned(),
        ]
        .into_iter()
        .flat_map(|option| ["-o".to_owned(), option])
        .collect(),
    )
}

/// `path` in double quotes for an SSH option: OpenSSH splits on spaces and
/// expands `%` tokens even in a command line argument, and runs
/// `ProxyCommand` through the shell, so what the shell expands inside
/// double quotes is refused too.
fn quoted(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if !path.is_absolute() || text.contains(['"', '\n', '\r', '\0', '$', '`', '\\']) {
        return None;
    }
    Some(format!("\"{}\"", text.replace('%', "%%")))
}

#[cfg(test)]
mod tests;
