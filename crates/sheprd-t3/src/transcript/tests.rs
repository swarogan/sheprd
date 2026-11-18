use super::*;
use crate::fixtures::{LIVE, values};
use serde_json::json;

fn recorded() -> Thread {
    let mut thread = Thread::default();
    for value in values(LIVE, "2") {
        thread.apply(value);
    }
    thread
}

#[test]
fn the_recorded_thread_reads_as_a_chat() {
    let transcript = transcript(&recorded(), "fakeacp");
    assert_eq!(transcript.agent, "fakeacp");
    assert_eq!(transcript.status, Status::Idle);
    let shown: Vec<String> = transcript
        .entries
        .iter()
        .map(|entry| match &entry.kind {
            EntryKind::User { text, .. } => format!("user: {text}"),
            EntryKind::Agent { text } => format!("agent: {}", text.lines().next().unwrap()),
            EntryKind::Tool(tool) => {
                format!("tool {:?}: {} {:?}", tool.kind, tool.title, tool.status)
            }
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        shown,
        [
            "user: Say hello and read the README",
            "tool Execute: Workspace ready Completed",
            "tool Other: Read README.md Completed",
            "agent: Echo: <t3_code_instructions>",
            "user: Second turn from Sheprd",
            "tool Other: Read README.md Completed",
            "agent: Echo: Second turn from Sheprd",
        ]
    );
    // The read's path is where the tool worked.
    let EntryKind::Tool(read) = &transcript.entries[2].kind else {
        panic!("a tool");
    };
    assert_eq!(read.locations, ["README.md"]);
}

/// A thread holding one turn item, with `message` for its text.
fn one(item: serde_json::Value, message: Option<serde_json::Value>) -> Thread {
    let mut thread = Thread::default();
    thread.apply(
        json!({"kind": "snapshot", "snapshotSequence": 1, "projection": {
        "thread": {"id": "t", "title": "T"},
        "messages": message.into_iter().collect::<Vec<_>>(),
        "turnItems": [item]}}),
    );
    thread
}

#[test]
fn an_item_without_its_text_takes_its_messages() {
    let thread = one(
        json!({"id": "i", "ordinal": 1, "type": "assistant_message", "messageId": "m",
            "textOmitted": true, "streaming": false}),
        Some(json!({"id": "m", "role": "assistant", "text": "From the message"})),
    );
    let transcript = transcript(&thread, "a");
    assert_eq!(
        transcript.entries[0].kind,
        EntryKind::Agent {
            text: "From the message".to_owned()
        }
    );
}

#[test]
fn failures_requests_and_long_output_read_plainly() {
    let failed = one(
        json!({"id": "i", "ordinal": 1, "type": "command_execution", "status": "completed",
            "input": "cargo test", "exitCode": 101, "output": "x".repeat(MAX_OUTPUT + 10)}),
        None,
    );
    let EntryKind::Tool(tool) = &transcript(&failed, "a").entries[0].kind else {
        panic!("a tool");
    };
    assert_eq!(tool.title, "cargo test");
    assert_eq!(tool.status, ToolStatus::Failed);
    let ToolContent::Text { text } = &tool.content[0] else {
        panic!("text output");
    };
    assert!(text.len() <= MAX_OUTPUT + "\n[cut]".len());
    assert!(text.ends_with("[cut]"));

    let asking = one(
        json!({"id": "i", "ordinal": 1, "type": "approval_request", "requestId": "r",
            "requestKind": "command", "prompt": "Run rm -rf build?"}),
        None,
    );
    assert_eq!(
        transcript(&asking, "a").entries[0].kind,
        EntryKind::Notice {
            answers: Vec::new(),
            details: None,
            level: NoticeLevel::Warning,
            text: "Run rm -rf build?".to_owned(),
        }
    );
    let error = one(
        json!({"id": "i", "ordinal": 1, "type": "error",
            "failure": {"message": "Usage limit reached"}}),
        None,
    );
    assert!(matches!(
        &transcript(&error, "a").entries[0].kind,
        EntryKind::Notice { level: NoticeLevel::Error, text, .. } if text == "Usage limit reached"
    ));
}
