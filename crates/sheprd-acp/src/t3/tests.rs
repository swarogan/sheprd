use super::*;
use sheprd_chat_protocol::{Change, EntryKind, ServerMessage};

/// The frames recorded while a second message ran on a T3 Code 0.1.7 thread.
fn recorded() -> Vec<ServerFrame> {
    let frames: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../../../sheprd-t3/fixtures/live-turn.json")).unwrap();
    frames
        .into_iter()
        .map(|frame| serde_json::from_value(frame).unwrap())
        .collect()
}

#[test]
fn a_live_turn_reaches_the_chat_as_it_happens_read_only() {
    // Request 2 was the thread's stream in the recording.
    let mut follower = Follower::new("2".to_owned());
    assert!(!follower.ready());
    let mut seen = Vec::new();
    for frame in recorded() {
        match follower.frame(frame) {
            Step::Changed(transcript) => seen.push(*transcript),
            Step::Same => {}
            Step::Ended(reason) => panic!("the stream ended: {reason}"),
        }
    }
    assert!(follower.ready());
    // The snapshot, then the turn's changes, each its own transcript.
    assert!(seen.len() > 2, "{} transcripts", seen.len());
    let first = &seen[0];
    let last = seen.last().unwrap();
    assert!(first.read_only && last.read_only);
    assert_eq!(last.agent, "fakeacp in T3 Code");
    assert!(last.entries.len() > first.entries.len());
    assert!(last.entries.iter().any(|entry| matches!(
        &entry.kind,
        EntryKind::Agent { text } if text.starts_with("Echo: Second turn from Sheprd")
    )));
    assert_eq!(last.status, Status::Idle);
    // Somewhere in between, the agent was working.
    assert!(
        seen.iter()
            .any(|transcript| transcript.status == Status::Working)
    );
}

#[test]
fn other_requests_frames_change_nothing_and_an_exit_ends_the_stream() {
    let mut follower = Follower::new("2".to_owned());
    let other = ServerFrame::Chunk {
        request_id: "1".to_owned(),
        values: vec![serde_json::json!({"kind": "synchronized"})],
    };
    assert_eq!(follower.frame(other), Step::Same);
    assert_eq!(follower.frame(ServerFrame::Pong), Step::Same);
    let refused: ServerFrame = serde_json::from_value(serde_json::json!({"_tag": "Exit",
        "requestId": "2", "exit": {"_tag": "Failure",
            "cause": [{"_tag": "Fail", "error": "Thread not found"}]}}))
    .unwrap();
    assert_eq!(
        follower.frame(refused),
        Step::Ended("Thread not found".to_owned())
    );
}

#[test]
fn an_attached_client_sees_the_threads_status_change() {
    let mut follower = Follower::new("2".to_owned());
    let mut seen = Vec::new();
    for frame in recorded() {
        if let Step::Changed(transcript) = follower.frame(frame) {
            seen.push(*transcript);
        }
    }
    let working = seen
        .iter()
        .find(|transcript| transcript.status == Status::Working)
        .unwrap()
        .clone();
    let mut mirror = Mirror::new(working.clone(), None);
    let (outbox, queue) = bounded(16);
    mirror.input(Input::Attach(crate::session::Attach {
        conn: crate::session::ConnId(1),
        local: false,
        outbox,
        since: None,
    }));
    let mut client = match (queue.recv().unwrap(), queue.recv().unwrap()) {
        (ServerMessage::Welcome { .. }, ServerMessage::Snapshot { transcript, .. }) => *transcript,
        other => panic!("{other:?}"),
    };
    let mut ended = working;
    ended.status = Status::Failed {
        message: "The T3 Code connection ended: gone".to_owned(),
    };
    mirror.update(ended.clone());
    let mut statuses = 0;
    while let Ok(message) = queue.try_recv() {
        let ServerMessage::Change { change, .. } = message else {
            panic!("a change, not {message:?}");
        };
        statuses += usize::from(matches!(change, Change::Status { .. }));
        client.apply(&change).unwrap();
    }
    assert_eq!(statuses, 1);
    assert_eq!(client.status, ended.status);
}

#[test]
fn a_server_that_never_answers_a_ping_ends_the_stream() {
    use std::{net::TcpListener, time::Duration};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        // Read everything, answer nothing, until the client goes.
        let mut pings = 0;
        while let Ok(message) = socket.read() {
            if let tungstenite::Message::Text(text) = message
                && text.as_str().contains("\"Ping\"")
            {
                pings += 1;
            }
        }
        pings
    });
    let stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let url = url::Url::parse(&format!("ws://127.0.0.1:{port}/ws")).unwrap();
    let connection = Connection::handshake(&url, stream).unwrap();
    let (updates, changes) = bounded(4);
    follow(connection, Follower::new("2".to_owned()), &updates);
    assert_eq!(
        changes.try_recv().unwrap(),
        Step::Ended("T3 Code stopped answering.".to_owned())
    );
    // The worker gave up after one unanswered ping, then let the socket go.
    assert_eq!(server.join().unwrap(), 1);
}
