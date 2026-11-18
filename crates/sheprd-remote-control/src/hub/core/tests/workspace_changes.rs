//! A phone's close of a workspace Sheprd keeps, or deletion of its
//! worktree, while Sheprd is closed: the daemon makes neither itself.

use super::*;
use crate::control::layout::Op;

#[test]
fn closing_or_deleting_a_kept_workspace_needs_sheprd_and_leaves_its_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let tabs = dir.path().join("browser-tabs.json");
    let saved = json!({"tabs": [], "workspaces": [
        {"id": "space-1", "scope": "local:/test.sock", "label": "api", "cwd": "/code/api"}
    ]})
    .to_string();
    std::fs::write(&tabs, &saved).unwrap();
    let source = Source {
        connected: AtomicBool::new(false),
        endpoint: Default::default(),
        holds: AtomicU64::new(0),
        own: Mutex::default(),
        poller: Mutex::default(),
        scope: Some("local:/test.sock".into()),
        state: Mutex::default(),
        tabs: Some(tabs.clone()),
    };
    for op in [
        Op::CloseWorkspace {
            workspace: "space-1".into(),
        },
        Op::RemoveWorktree {
            workspace: "space-1".into(),
        },
    ] {
        let error = source.arrange_saved(0, "", &op).unwrap_err();
        assert!(
            matches!(
                error,
                HubError::Core(Error::Arrange(arrange::Error::SheprdClosed))
            ),
            "{op:?}: {error:?}"
        );
        assert_eq!(std::fs::read_to_string(&tabs).unwrap(), saved);
        // The daemon's own tabs are never Sheprd's workspaces.
        assert!(source.arrange_own("t1", &op).is_err());
    }
}
