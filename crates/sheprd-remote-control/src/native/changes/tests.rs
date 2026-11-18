use super::*;

#[test]
fn workspace_ending_waits_allow_for_shutdown_and_checkout_cleanup() {
    assert_eq!(layout_wait(&layout::Op::Zoom), LAYOUT_WAIT);
    assert_eq!(
        layout_wait(&layout::Op::CloseWorkspace {
            workspace: "w1".into(),
        }),
        WORKSPACE_CLOSE_WAIT
    );
    assert_eq!(
        layout_wait(&layout::Op::RemoveWorktree {
            workspace: "w1".into(),
        }),
        WORKTREE_REMOVE_WAIT
    );
    assert!(WORKTREE_REMOVE_WAIT > Duration::from_secs(40));
}
