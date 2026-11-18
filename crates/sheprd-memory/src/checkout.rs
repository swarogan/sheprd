//! The checkout a directory is in, found once with one Git call, and its
//! branch read from `HEAD` whenever it is needed: a chat may outlive a
//! `git switch`, and reading a file costs far less than running Git again
//! for every prompt.
use crate::{Error, Result};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkout {
    /// The repository's Git common directory, absolute and canonical, which
    /// every worktree of it shares and memory is keyed by.
    pub common_dir: PathBuf,
    /// This worktree's own Git directory, whose `HEAD` names its branch.
    pub git_dir: PathBuf,
}

impl Checkout {
    /// The checkout `dir` is in.
    pub fn find(dir: &Path) -> Result<Self> {
        let output = sheprd_client::new_command("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--absolute-git-dir", "--git-common-dir"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|source| Error::Git {
                dir: dir.to_owned(),
                source,
            })?;
        if !output.status.success() {
            return Err(Error::NotInCheckout);
        }
        let text = String::from_utf8(output.stdout).map_err(|_| Error::NotInCheckout)?;
        let mut lines = text.lines();
        let (Some(git_dir), Some(common)) = (lines.next(), lines.next()) else {
            return Err(Error::NotInCheckout);
        };
        // Git may print the common directory relative to `dir`.
        let common_dir = std::fs::canonicalize(dir.join(common)).map_err(|source| Error::Git {
            dir: dir.to_owned(),
            source,
        })?;
        Ok(Self {
            common_dir,
            git_dir: PathBuf::from(git_dir),
        })
    }

    /// The branch checked out now, `None` when `HEAD` is detached or
    /// unreadable.
    pub fn branch(&self) -> Option<String> {
        let head = std::fs::read_to_string(self.git_dir.join("HEAD")).ok()?;
        branch_of(&head)
    }
}

/// The branch a `HEAD` file names.
pub(crate) fn branch_of(head: &str) -> Option<String> {
    head.trim_end()
        .strip_prefix("ref: refs/heads/")
        .filter(|branch| !branch.is_empty())
        .map(str::to_owned)
}
