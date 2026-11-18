//! What repository each listed workspace's checkout is on, for a phone to
//! read GitHub itself: its `upstream` and `origin` remotes, branch, the
//! remote that branch tracks, and commit. Read with Git on the blocking pool while a phone looks, never
//! while a home is drawn, which only looks the folder up here.

use super::remote::{self, Repository};
use crate::{native::changes::local::git, web::Shared};
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Notify, watch};

/// Folders read in one pass; more workspaces than this go without.
const MAX_CHECKOUTS: usize = 256;
/// How often the folders are read again while a phone looks.
const REFRESH: Duration = Duration::from_secs(30);
/// How long after a phone last asked for a home it still counts as looking.
const LOOKING: Duration = Duration::from_secs(120);
/// How soon after a phone asks a pass may run again.
const FRESH: Duration = Duration::from_secs(10);
/// The longest branch name kept.
const MAX_BRANCH: usize = 255;
/// The longest remote name read from a branch's `remote`.
const MAX_REMOTE: usize = 255;

/// What one checkout is on.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Checkout {
    pub(crate) origin: Option<Repository>,
    pub(crate) upstream: Option<Repository>,
    /// `None` on a detached HEAD.
    pub(crate) branch: Option<String>,
    /// `None` before the first commit.
    pub(crate) head: Option<String>,
    /// The repository of the remote the branch tracks
    /// (`branch.<name>.remote`); `None` when it tracks none, or one this
    /// parser does not read.
    pub(crate) tracked: Option<Repository>,
}

impl Checkout {
    /// The repository the branch is pushed to: the one it tracks, else
    /// `origin`.
    pub(crate) fn pushed(&self) -> Option<&Repository> {
        self.tracked.as_ref().or(self.origin.as_ref())
    }
}

/// The checkouts read, by folder, and when a phone last wanted them.
pub(crate) struct Checkouts {
    known: watch::Sender<Arc<HashMap<String, Checkout>>>,
    wanted: std::sync::Mutex<Option<Instant>>,
    wake: Notify,
}

impl Default for Checkouts {
    fn default() -> Self {
        Self {
            known: watch::Sender::new(Arc::default()),
            wanted: std::sync::Mutex::default(),
            wake: Notify::new(),
        }
    }
}

impl Checkouts {
    /// The checkouts as last read.
    pub(crate) fn current(&self) -> Arc<HashMap<String, Checkout>> {
        self.known.borrow().clone()
    }

    /// Changes whenever a read finds something new.
    pub(crate) fn watch(&self) -> watch::Receiver<Arc<HashMap<String, Checkout>>> {
        self.known.subscribe()
    }

    /// A phone asked for a home: read the folders soon, and for a while.
    pub(crate) fn want(&self) {
        let mut wanted = self
            .wanted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        if wanted.is_none_or(|at| now.saturating_duration_since(at) >= FRESH) {
            self.wake.notify_one();
        }
        *wanted = Some(now);
    }

    fn looking(&self) -> bool {
        self.wanted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some_and(|at| at.elapsed() < LOOKING)
    }

    #[cfg(test)]
    pub(crate) fn set_for_tests(&self, known: HashMap<String, Checkout>) {
        self.known.send_replace(Arc::new(known));
    }
}

/// Reads the listed folders again whenever a phone looks: when one asks,
/// when the layout changes, and every [`REFRESH`] while it keeps looking.
pub(crate) async fn follow(state: Shared) {
    let mut layout = state.hub.watch_layout();
    loop {
        let checkouts = &state.lending.checkouts;
        if checkouts.looking() || !state.connected().is_empty() {
            let mut folders: Vec<String> = state
                .hub
                .layout()
                .workspaces
                .iter()
                // A WSL folder is not this computer's to read with its own
                // Git; those workspaces name no repository, and the phone
                // asks the computer about them instead.
                .filter(|workspace| !crate::hub::in_wsl(&workspace.id))
                .map(|workspace| workspace.cwd.clone())
                .filter(|cwd| !cwd.is_empty() && cwd.len() <= 4096)
                .collect();
            folders.sort_unstable();
            folders.dedup();
            folders.truncate(MAX_CHECKOUTS);
            let read = tokio::task::spawn_blocking(move || {
                folders
                    .into_iter()
                    .filter_map(|cwd| Some((read(Path::new(&cwd))?, cwd)))
                    .map(|(checkout, cwd)| (cwd, checkout))
                    .collect::<HashMap<_, _>>()
            })
            .await;
            if let Ok(read) = read {
                checkouts.known.send_if_modified(|known| {
                    if **known == read {
                        return false;
                    }
                    *known = Arc::new(read);
                    true
                });
            }
        }
        tokio::select! {
            () = state.lending.checkouts.wake.notified() => {}
            changed = layout.changed() => if changed.is_err() { return },
            () = tokio::time::sleep(REFRESH) => {}
        }
        // Let a burst of layout changes settle into one pass.
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// What the checkout at `cwd` is on; `None` when it is not a Git checkout
/// with an `origin` or `upstream` remote. Blocking.
pub(crate) fn read(cwd: &Path) -> Option<Checkout> {
    let remotes = git(
        cwd,
        &[
            "config",
            "--get-regexp",
            r"^remote\.(origin|upstream)\.url$",
        ],
    )
    .ok()?;
    let mut checkout = Checkout::default();
    for line in remotes.lines() {
        let Some((key, url)) = line.split_once(' ') else {
            continue;
        };
        let slot = match key {
            "remote.origin.url" => &mut checkout.origin,
            "remote.upstream.url" => &mut checkout.upstream,
            _ => continue,
        };
        // The last value wins, as Git's own `--get` does.
        *slot = remote::parse(url);
    }
    if checkout.origin.is_none() && checkout.upstream.is_none() {
        return None;
    }
    (checkout.branch, checkout.head) = head(cwd);
    checkout.tracked = checkout
        .branch
        .as_deref()
        .and_then(|branch| tracked(cwd, branch, &checkout));
    Some(checkout)
}

/// The repository of the remote `branch` tracks. `origin` and `upstream`
/// were read already; another remote costs one more Git call. A branch
/// tracking a local one (`.`) tracks no repository.
fn tracked(cwd: &Path, branch: &str, checkout: &Checkout) -> Option<Repository> {
    let remote = git(
        cwd,
        &["config", "--get", &format!("branch.{branch}.remote")],
    )
    .ok()?;
    let remote = remote.trim();
    match remote {
        "origin" => checkout.origin.clone(),
        "upstream" => checkout.upstream.clone(),
        "" | "." => None,
        _ if remote.len() > MAX_REMOTE
            || remote
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control()) =>
        {
            None
        }
        _ => {
            let url = git(cwd, &["config", "--get", &format!("remote.{remote}.url")]).ok()?;
            // `--get` prints the last value, as Git itself uses.
            remote::parse(url.trim())
        }
    }
}

/// The branch checked out, unless HEAD is detached, and its commit.
fn head(cwd: &Path) -> (Option<String>, Option<String>) {
    let branch = git(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .map(|branch| branch.trim().to_owned())
        .filter(|branch| !branch.is_empty() && branch.len() <= MAX_BRANCH);
    let head = git(cwd, &["rev-parse", "--verify", "--quiet", "HEAD"])
        .ok()
        .map(|head| head.trim().to_owned())
        .filter(|head| {
            matches!(head.len(), 40 | 64) && head.bytes().all(|b| b.is_ascii_hexdigit())
        });
    (branch, head)
}
