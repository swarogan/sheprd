//! A phone's close of a workspace Sheprd keeps, or deletion of its worktree.

use crate::control::{
    changes::AskId,
    layout::{Ask, Failure, MAX_TERMINAL, Op},
};

fn ask(op: Op) -> Ask {
    Ask {
        id: AskId(1),
        tab: 0,
        terminal: String::new(),
        op,
    }
}

#[test]
fn a_workspace_change_names_its_workspace_within_bounds() {
    for op in [
        |workspace: String| Op::CloseWorkspace { workspace },
        |workspace: String| Op::RemoveWorktree { workspace },
    ] {
        assert!(ask(op("space-2".into())).valid());
        assert!(!ask(op(String::new())).valid());
        assert!(!ask(op("x".repeat(MAX_TERMINAL + 1))).valid());
    }
}

#[test]
fn workspace_changes_read_on_the_wire_as_named() {
    let wire = serde_json::to_value(Op::CloseWorkspace {
        workspace: "space-2".into(),
    })
    .unwrap();
    assert_eq!(
        wire,
        serde_json::json!({"close_workspace": {"workspace": "space-2"}})
    );
    let wire = serde_json::to_value(Op::RemoveWorktree {
        workspace: "space-2".into(),
    })
    .unwrap();
    assert_eq!(
        wire,
        serde_json::json!({"remove_worktree": {"workspace": "space-2"}})
    );
}

#[test]
fn a_deletion_refusal_reads_as_itself_and_an_unknown_one_as_refused() {
    let dirty: Failure = serde_json::from_value(serde_json::json!("dirty")).unwrap();
    assert_eq!(dirty, Failure::Dirty);
    let gone: Failure = serde_json::from_value(serde_json::json!("not_worktree")).unwrap();
    assert_eq!(gone, Failure::NotWorktree);
    let newer: Failure = serde_json::from_value(serde_json::json!("from_the_future")).unwrap();
    assert_eq!(newer, Failure::Refused);
}
