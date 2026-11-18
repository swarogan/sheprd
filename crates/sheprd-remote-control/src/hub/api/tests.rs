use super::*;

#[test]
fn workspace_ending_requests_allow_for_cleanup() {
    assert_eq!(ApiMethod::PaneClose.timeout(), TIMEOUT);
    assert_eq!(ApiMethod::WorkspaceClose.timeout(), WORKSPACE_CLOSE_TIMEOUT);
    assert_eq!(ApiMethod::WorktreeRemove.timeout(), WORKTREE_REMOVE_TIMEOUT);
}
