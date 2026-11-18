use super::*;
use crate::fixtures::{LIVE, SNAPSHOTS, THREAD, values};
use serde_json::json;

#[test]
fn the_recorded_list_has_its_thread_and_project() {
    let mut shell = Shell::default();
    for value in values(SNAPSHOTS, "1") {
        shell.apply(value).unwrap();
    }
    let thread = &shell.threads[THREAD];
    assert_eq!(thread.title, "Echo test");
    assert_eq!(thread.provider_instance_id.as_deref(), Some("fakeacp"));
    assert_eq!(thread.status.as_deref(), Some("completed"));
    assert!(!thread.waiting());
    assert_eq!(shell.projects[&thread.project_id].title, "Sandbox");
}

#[test]
fn a_project_refresh_keeps_the_threads_it_leaves_out() {
    // T3 sends a list, then a refresh of its projects' details alone, with
    // no threads; often in one chunk.
    let mut shell = Shell::default();
    let values = values(LIVE, "1");
    assert!(values.len() >= 2);
    for value in values {
        shell.apply(value).unwrap();
    }
    assert!(shell.threads.contains_key(THREAD));
    let sequence = shell.sequence;
    let refresh = json!({"kind": "snapshot", "resolvedRepositoryIdentityRoots": ["/w"],
        "snapshot": {"schemaVersion": 2, "snapshotSequence": 999, "threads": [],
            "projects": [{"id": "4e50c501-307a-4fd9-84b9-6f6e0391c8ae",
                "title": "Renamed", "workspaceRoot": "/tmp/t3-sandbox/work"}]}});
    shell.apply(refresh).unwrap();
    assert!(shell.threads.contains_key(THREAD));
    assert_eq!(shell.sequence, sequence);
    assert_eq!(
        shell.projects["4e50c501-307a-4fd9-84b9-6f6e0391c8ae"].title,
        "Renamed"
    );
}

#[test]
fn changes_older_than_the_list_are_ignored() {
    let mut shell = Shell::default();
    for value in values(SNAPSHOTS, "1") {
        shell.apply(value).unwrap();
    }
    let current = shell.sequence.unwrap();
    let stale = json!({"kind": "thread.updated", "sequence": current, "location": "active",
        "thread": {"id": THREAD, "projectId": "p", "title": "Stale"}});
    shell.apply(stale).unwrap();
    assert_eq!(shell.threads[THREAD].title, "Echo test");
    let newer = json!({"kind": "thread.removed", "sequence": current + 1,
        "location": "active", "threadId": THREAD});
    shell.apply(newer).unwrap();
    assert!(shell.threads.is_empty());
    assert_eq!(shell.sequence, Some(current + 1));
}

#[test]
fn another_schema_is_refused_and_unknown_kinds_are_skipped() {
    let mut shell = Shell::default();
    let error = shell
        .apply(json!({"kind": "snapshot", "snapshot":
            {"schemaVersion": 3, "snapshotSequence": 1, "threads": [], "projects": []}}))
        .unwrap_err();
    assert!(matches!(
        error,
        crate::Error::Protocol {
            expected: 2,
            found: 3
        }
    ));
    shell
        .apply(json!({"kind": "thread.pinned", "sequence": 5}))
        .unwrap();
    shell.apply(json!("not an item")).unwrap();
    assert_eq!(shell, Shell::default());
}
