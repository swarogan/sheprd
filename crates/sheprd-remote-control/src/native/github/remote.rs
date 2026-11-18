//! The host and repository a Git remote URL names: scp-style SSH
//! (`git@host:owner/repo.git`), `ssh://`, and `https://`. Whether the host
//! is a GitHub host is [`super::hosts`]'s to say. A user name or token in
//! the URL is skipped and never kept.

use crate::control::github::valid_host;

/// A bound on each path segment, above GitHub's own name limits.
const MAX_SEGMENT: usize = 100;

/// A repository on some Git host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Repository {
    /// Lowercase, without a port.
    pub(crate) host: String,
    pub(crate) owner: String,
    pub(crate) repo: String,
}

/// The repository `url` names, when it is one this parser reads.
pub(crate) fn parse(url: &str) -> Option<Repository> {
    let url = url.trim();
    let (host, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/')?;
            let host = authority_host(authority)?;
            match scheme.to_ascii_lowercase().as_str() {
                // The SSH port says nothing about the API's.
                "ssh" | "git+ssh" | "ssh+git" => (strip_port(host)?, path),
                // GitHub's API answers on the default port only.
                "https" | "http" if !host.contains(':') => (host, path),
                _ => return None,
            }
        }
        None => scp(url)?,
    };
    let host = host.to_ascii_lowercase();
    if !valid_host(&host) {
        return None;
    }
    let path = path.trim_matches('/');
    let (owner, repo) = path.split_once('/')?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    (segment(owner) && segment(repo)).then(|| Repository {
        host,
        owner: owner.to_owned(),
        repo: repo.to_owned(),
    })
}

/// `[user@]host:path`, Git's scp-like syntax: a colon before any slash.
fn scp(url: &str) -> Option<(&str, &str)> {
    let (authority, path) = url.split_once(':')?;
    if authority.contains('/') || path.starts_with("//") {
        return None;
    }
    Some((authority_host(authority)?, path))
}

/// The host of `[userinfo@]host[:port]`. Userinfo that could be misread as
/// the end of the authority is refused, so the host is never misread.
fn authority_host(authority: &str) -> Option<&str> {
    let host = match authority.rsplit_once('@') {
        None => authority,
        Some((userinfo, host)) => {
            let plain = |b: u8| b.is_ascii_graphic() && !matches!(b, b'@' | b'?' | b'#' | b'\\');
            if userinfo.is_empty() || !userinfo.bytes().all(plain) {
                return None;
            }
            host
        }
    };
    (!host.is_empty()).then_some(host)
}

/// `host` without a numeric `:port`.
fn strip_port(host: &str) -> Option<&str> {
    match host.split_once(':') {
        None => Some(host),
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            Some(host)
        }
        Some(_) => None,
    }
}

/// One owner or repository name: no name grammar is reproduced, but nothing
/// that would leave its own segment of an API path.
fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SEGMENT
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
