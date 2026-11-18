//! Updating a checkout selected from the repository's registered worktrees.

use super::{Host, Result, Step, script, shell_quote};
use std::sync::atomic::AtomicBool;

/// Pull an existing checkout without rewriting local commits or stashing edits.
/// `remote` names a PR's remote and source ref (including fork PR refs); otherwise
/// Git uses the branch's configured upstream. Runs on the checkout's host.
pub fn pull_existing(
    host: &Host,
    repository: &str,
    path: &str,
    branch: &str,
    remote: Option<(&str, &str)>,
    cancelled: &AtomicBool,
) -> Result<()> {
    let remote = remote.map_or_else(String::new, |(remote, reference)| {
        format!(" {} {}", shell_quote(remote), shell_quote(reference))
    });
    // Recheck identity immediately before updating: a listing may have aged
    // while the user or another client changed the checkout.
    let line = format!(
        r#"repo={repository}
tree={path}
common=$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)
[ "$(git -C "$tree" rev-parse --path-format=absolute --git-common-dir)" = "$common" ]
[ "$(git -C "$tree" symbolic-ref --quiet HEAD)" = {branch} ]
export GIT_TERMINAL_PROMPT=0 GCM_INTERACTIVE=never SSH_ASKPASS_REQUIRE=never
git -c core.fsmonitor=false -C "$tree" pull --ff-only --no-rebase --no-autostash{remote}
"#,
        repository = shell_quote(repository),
        path = shell_quote(path),
        branch = shell_quote(&format!("refs/heads/{branch}")),
    );
    host.capture(&line, cancelled)
        .map(drop)
        .map_err(script(Step::Pull))
}
