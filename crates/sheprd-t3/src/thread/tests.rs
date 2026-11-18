use super::*;
use crate::fixtures::{LIVE, SNAPSHOTS, THREAD, values};
use serde_json::json;

fn recorded(text: &str) -> Thread {
    let mut thread = Thread::default();
    for value in values(text, "2") {
        thread.apply(value);
    }
    thread
}

fn kinds(thread: &Thread) -> Vec<&'static str> {
    thread
        .items()
        .into_iter()
        .map(|(_, item)| match item.kind {
            TurnKind::UserMessage { .. } => "user",
            TurnKind::AssistantMessage { .. } => "assistant",
            TurnKind::CommandExecution { .. } => "command",
            TurnKind::DynamicTool { .. } => "tool",
            TurnKind::Other => "other",
            _ => "?",
        })
        .collect()
}

#[test]
fn the_recorded_thread_is_its_turn_items_in_order() {
    let thread = recorded(SNAPSHOTS);
    assert_eq!(thread.header.id, THREAD);
    assert_eq!(thread.header.title, "Echo test");
    assert_eq!(
        kinds(&thread),
        ["user", "command", "tool", "assistant", "other"]
    );
    assert!(!thread.working());
    assert!(!thread.waiting());
}

#[test]
fn a_live_turn_adds_its_items_and_keeps_earlier_entry_ids() {
    let before = recorded(SNAPSHOTS);
    let after = recorded(LIVE);
    assert_eq!(
        kinds(&after),
        [
            "user",
            "command",
            "tool",
            "assistant",
            "other",
            "user",
            "tool",
            "assistant",
            "other"
        ]
    );
    let ids = |thread: &Thread| -> Vec<EntryId> {
        thread.items().into_iter().map(|(id, _)| id).collect()
    };
    assert_eq!(ids(&after)[..5], ids(&before)[..]);
    // The turn finished within the recording.
    assert!(!after.working());
    assert!(after.sequence > before.sequence);
}

#[test]
fn a_run_under_way_is_working_and_a_pending_request_waits() {
    let mut thread = recorded(SNAPSHOTS);
    let next = thread.sequence.unwrap() + 1;
    thread.apply(
        json!({"kind": "event", "sequence": next, "event": {"type": "run.updated",
        "payload": {"id": "run-2", "status": "running"}}}),
    );
    assert!(thread.working());
    thread.apply(json!({"kind": "event", "sequence": next + 1, "event": {
        "type": "runtime-request.updated", "payload": {"id": "req-1", "status": "pending"}}}));
    assert!(thread.waiting());
    thread.apply(json!({"kind": "event", "sequence": next + 2, "event": {
        "type": "runtime-request.updated", "payload": {"id": "req-1", "status": "resolved"}}}));
    assert!(!thread.waiting());
}

#[test]
fn old_events_are_ignored_and_unknown_ones_move_the_sequence() {
    let mut thread = recorded(SNAPSHOTS);
    let current = thread.sequence.unwrap();
    thread.apply(
        json!({"kind": "event", "sequence": current, "event": {"type": "run.updated",
        "payload": {"id": "run-2", "status": "running"}}}),
    );
    assert!(!thread.working());
    thread.apply(json!({"kind": "event", "sequence": current + 5,
        "event": {"type": "thread.goal-set", "payload": {}}}));
    assert_eq!(thread.sequence, Some(current + 5));
    thread.apply(json!({"kind": "synchronized"}));
    thread.apply(json!(42));
    assert_eq!(thread.sequence, Some(current + 5));
}

#[test]
fn a_turn_item_of_an_unknown_type_is_kept_as_other() {
    let mut thread = recorded(SNAPSHOTS);
    let next = thread.sequence.unwrap() + 1;
    thread.apply(json!({"kind": "event", "sequence": next, "event": {
        "type": "turn-item.updated", "payload": {"id": "new", "ordinal": 9_999_999,
            "type": "hologram", "status": "completed"}}}));
    assert_eq!(kinds(&thread).last(), Some(&"other"));
}
