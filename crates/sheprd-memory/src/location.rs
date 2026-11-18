//! Where a repository's memory lives: `repositories/<uuid v5>/memory.sqlite3`
//! in the app's state directory, beside the orchestrator's database for the
//! same repository and keyed the same way, by its Git common directory.
//!
//! It is a file of its own rather than a table of the orchestrator's
//! `state.sqlite3`, whose schema agent-launcher shares: a new table there
//! would raise that schema's version and leave older agent-launcher builds
//! read-only.
use std::path::{Path, PathBuf};
use uuid::Uuid;

const FILE: &str = "memory.sqlite3";

/// The memory file of the repository whose Git common directory is
/// `git_dir` (absolute and canonical), or `None` without a state directory.
pub fn path(git_dir: &Path) -> Option<PathBuf> {
    Some(path_in(&sheprd_client::local_path::state_dir()?, git_dir))
}

/// The memory file of `git_dir`'s repository under the state directory
/// `state`. The folder name is the orchestrator's `Repository::Local` ID.
pub fn path_in(state: &Path, git_dir: &Path) -> PathBuf {
    let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, git_dir.to_string_lossy().as_bytes());
    state
        .join("repositories")
        .join(id.hyphenated().to_string())
        .join(FILE)
}
