//! Switching agents in the middle of a conversation: the agent taking over
//! is handed it with the next prompt, and one switched back to resumes its
//! own session and hears only what happened since.
use super::harnesses::{choosing, launch, requests, set_agent};
use super::*;

fn respond_as(harness: &mut Harness, agent: u64, id: i64, result: Value) {
    harness.input(Input::Agent(
        HarnessId(agent),
        Inbound::Response {
            id: wire::RequestId::Number(id),
            result: Ok(result),
        },
    ));
}

fn fail_as(harness: &mut Harness, agent: u64, id: i64, message: &str) {
    harness.input(Input::Agent(
        HarnessId(agent),
        Inbound::Response {
            id: wire::RequestId::Number(id),
            result: Err(wire::Error::internal_error().data(message)),
        },
    ));
}

fn say_as(harness: &mut Harness, agent: u64, session: &str, update: Value) {
    harness.input(Input::Agent(
        HarnessId(agent),
        Inbound::Notification {
            method: "session/update".to_owned(),
            params: json!({"sessionId": session, "update": update}),
        },
    ));
}

fn reply(text: &str) -> Value {
    json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": text}})
}

/// Starts the agent the last switch asked for as `agent`, answering its
/// handshake with `capabilities`, and returns its outbox and the request
/// opening its session.
fn start(harness: &mut Harness, agent: u64, capabilities: Value) -> (Receiver<Vec<u8>>, Value) {
    let outbox = launch(harness);
    let initialize = requests(&outbox)[0]["id"].as_i64().unwrap();
    respond_as(
        harness,
        agent,
        initialize,
        json!({"protocolVersion": 1, "agentCapabilities": capabilities}),
    );
    let open = requests(&outbox).remove(0);
    (outbox, open)
}

/// The prompt blocks the agent behind `outbox` received for `text`.
fn sent_prompt(
    harness: &mut Harness,
    outbox: &Receiver<Vec<u8>>,
    text: &str,
) -> (i64, Vec<String>) {
    harness.command(
        1,
        Command::Prompt {
            text: text.to_owned(),
            attachments: Vec::new(),
            origin: None,
        },
    );
    let request = requests(outbox)
        .into_iter()
        .find(|request| request["method"] == "session/prompt")
        .expect("the prompt reached the agent");
    let blocks = request["params"]["prompt"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["text"].as_str().unwrap_or_default().to_owned())
        .collect();
    (request["id"].as_i64().unwrap(), blocks)
}

/// Claude answered "hello" with "Hi there", then the chat switched to Codex,
/// which opened session `c1`; returns Codex's outbox.
fn handed_to_codex(harness: &mut Harness) -> Receiver<Vec<u8>> {
    let turn = prompt(harness, 1, "hello");
    harness.agent_says("Hi there");
    harness.respond(turn, json!({"stopReason": "end_turn"}));
    let (flow, _) = set_agent(harness, Agent::Codex);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Codex,
            harness: HarnessId(1),
            load: None,
        }
    );
    let (codex, open) = start(harness, 1, json!({}));
    assert_eq!(open["method"], "session/new");
    respond_as(
        harness,
        1,
        open["id"].as_i64().unwrap(),
        json!({"sessionId": "c1"}),
    );
    codex
}

#[test]
fn the_agent_taking_over_gets_the_conversation_with_the_next_prompt_only() {
    let mut harness = choosing();
    let codex = handed_to_codex(&mut harness);
    // The conversation stays on screen, marked where Codex took over.
    assert!(harness.transcript().entries.iter().any(|entry| matches!(
        &entry.kind,
        EntryKind::User { text, .. } if text == "hello"
    )));
    let notices = harness.notices();
    assert!(notices.iter().any(|(_, text)| text == "Switched to Codex."));
    assert!(notices.iter().any(|(_, text)| text
        == "Codex continues this conversation and gets the conversation so far with your next prompt."));

    let (turn, blocks) = sent_prompt(&mut harness, &codex, "next");
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].contains("with Claude"));
    assert!(blocks[0].contains("| User: hello\n| Agent: Hi there\n"));
    assert_eq!(blocks[1], "next");
    // What the user typed is what the transcript shows.
    assert!(!harness.transcript().entries.iter().any(|entry| matches!(
        &entry.kind,
        EntryKind::User { text, .. } if text.contains("Context from Sheprd")
    )));
    respond_as(&mut harness, 1, turn, json!({"stopReason": "end_turn"}));
    let (_, blocks) = sent_prompt(&mut harness, &codex, "and again");
    assert_eq!(blocks, ["and again"]);
}

#[test]
fn an_agent_switched_back_to_resumes_its_session_and_hears_only_what_followed() {
    let mut harness = choosing();
    let codex = handed_to_codex(&mut harness);
    let (turn, _) = sent_prompt(&mut harness, &codex, "next");
    say_as(&mut harness, 1, "c1", reply("Done by Codex"));
    respond_as(&mut harness, 1, turn, json!({"stopReason": "end_turn"}));

    let (flow, _) = set_agent(&mut harness, Agent::Claude);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Claude,
            harness: HarnessId(2),
            load: Some("s1".to_owned()),
        }
    );
    let (claude, open) = start(&mut harness, 2, json!({"loadSession": true}));
    assert_eq!(open["method"], "session/load");
    assert_eq!(open["params"]["sessionId"], "s1");
    // Its replay of what it already said adds nothing to the transcript.
    let entries = harness.transcript().entries.len();
    say_as(&mut harness, 2, "s1", reply("Hi there"));
    assert_eq!(harness.transcript().entries.len(), entries);
    respond_as(&mut harness, 2, open["id"].as_i64().unwrap(), json!({}));
    assert!(harness.notices().iter().any(|(_, text)| text
        == "Claude continues this conversation and gets what happened since it last took part with your next prompt."));

    let (_, blocks) = sent_prompt(&mut harness, &claude, "back to you");
    assert!(blocks[0].contains("your earlier session"));
    assert!(blocks[0].contains("with Codex"));
    assert!(blocks[0].contains("| User: next\n| Agent: Done by Codex\n"));
    assert!(!blocks[0].contains("User: hello"));
    assert_eq!(blocks[1], "back to you");
}

#[test]
fn an_agent_that_cannot_resume_starts_anew_with_the_whole_conversation() {
    for loads in [false, true] {
        let mut harness = choosing();
        let codex = handed_to_codex(&mut harness);
        let (turn, _) = sent_prompt(&mut harness, &codex, "next");
        respond_as(&mut harness, 1, turn, json!({"stopReason": "end_turn"}));
        set_agent(&mut harness, Agent::Claude);
        let (claude, open) = start(&mut harness, 2, json!({"loadSession": loads}));
        let new = if loads {
            // It offers to continue sessions, but this one is gone.
            assert_eq!(open["method"], "session/load");
            fail_as(
                &mut harness,
                2,
                open["id"].as_i64().unwrap(),
                "no such session",
            );
            requests(&claude).remove(0)
        } else {
            open
        };
        assert_eq!(new["method"], "session/new");
        respond_as(
            &mut harness,
            2,
            new["id"].as_i64().unwrap(),
            json!({"sessionId": "s2"}),
        );
        assert!(
            harness
                .notices()
                .iter()
                .any(|(level, _)| *level == NoticeLevel::Warning)
        );
        let (_, blocks) = sent_prompt(&mut harness, &claude, "back to you");
        assert!(blocks[0].contains("taking over a conversation"));
        assert!(blocks[0].contains("User: hello"));
        assert!(blocks[0].contains("User: next"));
    }
}

#[test]
fn an_agent_left_before_its_handoff_reached_it_is_handed_everything_again() {
    let mut harness = choosing();
    let _codex = handed_to_codex(&mut harness);
    // Back to Claude before Codex was ever prompted.
    let (flow, _) = set_agent(&mut harness, Agent::Claude);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Claude,
            harness: HarnessId(2),
            load: Some("s1".to_owned()),
        }
    );
    let (claude, open) = start(&mut harness, 2, json!({"loadSession": true}));
    respond_as(&mut harness, 2, open["id"].as_i64().unwrap(), json!({}));
    let (turn, _) = sent_prompt(&mut harness, &claude, "more");
    say_as(&mut harness, 2, "s1", reply("More from Claude"));
    respond_as(&mut harness, 2, turn, json!({"stopReason": "end_turn"}));

    // Codex's session never heard the conversation, so it is not resumed
    // as if it had: it starts anew and gets all of it.
    let (flow, _) = set_agent(&mut harness, Agent::Codex);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Codex,
            harness: HarnessId(3),
            load: None,
        }
    );
    let (codex, open) = start(&mut harness, 3, json!({"loadSession": true}));
    assert_eq!(open["method"], "session/new");
    respond_as(
        &mut harness,
        3,
        open["id"].as_i64().unwrap(),
        json!({"sessionId": "c2"}),
    );
    let (_, blocks) = sent_prompt(&mut harness, &codex, "your turn");
    assert!(blocks[0].contains("taking over a conversation"));
    assert!(blocks[0].contains("User: hello"));
    assert!(blocks[0].contains("User: more"));
}

#[test]
fn an_agent_that_failed_to_start_still_resumes_its_session_later() {
    let mut harness = choosing();
    let codex = handed_to_codex(&mut harness);
    let (turn, _) = sent_prompt(&mut harness, &codex, "next");
    respond_as(&mut harness, 1, turn, json!({"stopReason": "end_turn"}));
    set_agent(&mut harness, Agent::Claude);
    let error = crate::Error::AgentSpawn {
        program: "claude-agent-acp".to_owned(),
        source: std::io::Error::from(std::io::ErrorKind::NotFound),
    };
    harness.session.launch_failed(&error);

    let (flow, _) = set_agent(&mut harness, Agent::Codex);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Codex,
            harness: HarnessId(3),
            load: Some("c1".to_owned()),
        }
    );
    let (flow, _) = set_agent(&mut harness, Agent::Claude);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Claude,
            harness: HarnessId(4),
            load: Some("s1".to_owned()),
        }
    );
}

#[test]
fn a_turn_the_agent_rejects_leaves_the_handoff_for_the_next_prompt() {
    let mut harness = choosing();
    let codex = handed_to_codex(&mut harness);
    let (turn, blocks) = sent_prompt(&mut harness, &codex, "next");
    assert!(blocks[0].contains("User: hello"));
    fail_as(&mut harness, 1, turn, "turn/start failed");

    let (turn, blocks) = sent_prompt(&mut harness, &codex, "retry");
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].contains("User: hello"));
    assert_eq!(blocks[1], "retry");
    respond_as(&mut harness, 1, turn, json!({"stopReason": "end_turn"}));
    let (_, blocks) = sent_prompt(&mut harness, &codex, "and again");
    assert_eq!(blocks, ["and again"]);
}
