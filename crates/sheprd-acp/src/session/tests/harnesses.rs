use super::*;

/// A `--choose` chat offering Claude and Codex, whose Claude answered the
/// handshake and opened session `s1` with two modes.
pub(super) fn choosing() -> Harness {
    let mut harness = harness_with(Config {
        choices: Some(vec![Agent::Claude, Agent::Codex]),
        ..config()
    });
    harness.session.start();
    let initialize = harness.last_request("initialize")["id"].as_i64().unwrap();
    harness.respond(
        initialize,
        json!({"protocolVersion": 1, "agentCapabilities": {}}),
    );
    let new = harness.last_request("session/new")["id"].as_i64().unwrap();
    harness.respond(
        new,
        json!({"sessionId": "s1", "modes": {"currentModeId": "default", "availableModes": [
            {"id": "default", "name": "Default"}, {"id": "plan", "name": "Plan"}]}}),
    );
    harness
}

pub(super) fn set_agent(harness: &mut Harness, agent: Agent) -> (Flow, CommandId) {
    let id = CommandId::random();
    let flow = harness.input(Input::Command {
        command: Command::SetAgent { agent },
        conn: ConnId(1),
        id,
    });
    (flow, id)
}

pub(super) fn rejection(messages: &[ServerMessage], id: CommandId) -> Option<Rejection> {
    messages
        .iter()
        .find_map(|message| match message {
            ServerMessage::Ack {
                id: acked,
                rejected,
            } if *acked == id => Some(*rejected),
            _ => None,
        })
        .expect("the command was acknowledged")
}

/// The process the agent started second runs as.
const SECOND_PID: u32 = 4242;

/// Hands the session a new agent's outbox, as the program does after
/// [`Flow::Launch`], and returns what the session writes to it.
pub(super) fn launch(harness: &mut Harness) -> Receiver<Vec<u8>> {
    let (sender, agent) = bounded(64);
    harness.session.launched(Outbox(sender), SECOND_PID);
    agent
}

/// Answers request `id` as the agent started second.
fn respond_second(harness: &mut Harness, id: i64, result: Value) {
    harness.input(Input::Agent(
        HarnessId(1),
        Inbound::Response {
            id: wire::RequestId::Number(id),
            result: Ok(result),
        },
    ));
}

pub(super) fn requests(agent: &Receiver<Vec<u8>>) -> Vec<Value> {
    agent
        .try_iter()
        .map(|line| serde_json::from_slice(&line).unwrap())
        .collect()
}

#[test]
fn a_chosen_chat_offers_its_agents_and_a_fixed_one_does_not() {
    let harness = choosing();
    assert_eq!(
        harness.transcript().harnesses,
        Some(Harnesses {
            available: vec![Agent::Claude, Agent::Codex],
            current: Some(Agent::Claude),
        })
    );
    let mut fixed = ready();
    assert_eq!(fixed.transcript().harnesses, None);
    let client = fixed.subscribe(1, false, None);
    let (flow, id) = set_agent(&mut fixed, Agent::Codex);
    assert_eq!(flow, Flow::Continue);
    assert_eq!(rejection(&drain(&client), id), Some(Rejection::AgentFixed));
}

#[test]
fn switching_starts_the_new_agent_from_a_fresh_transcript() {
    let mut harness = choosing();
    let client = harness.subscribe(1, false, None);
    let before = harness.session.cursor();
    drain(&client);

    let (flow, id) = set_agent(&mut harness, Agent::Codex);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Codex,
            harness: HarnessId(1),
            load: None,
        }
    );
    assert_eq!(*harness.switched.borrow(), [Agent::Codex]);
    // The restore record follows the new agent only once it runs.
    assert!(harness.started.borrow().is_empty());
    let messages = drain(&client);
    assert_eq!(rejection(&messages, id), None);
    let Some(ServerMessage::Snapshot { cursor, transcript }) = messages.first() else {
        panic!("the client starts over from a snapshot: {messages:?}");
    };
    assert_ne!(cursor.epoch, before.epoch);
    assert_eq!(transcript.agent, "Codex");
    assert_eq!(transcript.status, Status::Starting);
    assert_eq!(transcript.modes, None);
    assert_eq!(transcript.session, None);
    assert_eq!(transcript.clients, 1);
    assert_eq!(
        transcript.harnesses.as_ref().and_then(|h| h.current),
        Some(Agent::Codex)
    );

    let agent = launch(&mut harness);
    let sent = requests(&agent);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["method"], "initialize");
    assert_eq!(*harness.started.borrow(), [SECOND_PID]);
    // Nothing more reached the agent switched away from.
    assert!(harness.sent().is_empty());
}

#[test]
fn the_old_agent_is_no_longer_heard() {
    let mut harness = choosing();
    set_agent(&mut harness, Agent::Codex);
    let _agent = launch(&mut harness);
    harness.agent_says("from the old agent");
    harness.input(Input::AgentExited {
        code: Some(1),
        harness: HarnessId(0),
        stderr: vec!["gone".to_owned()],
    });
    assert!(harness.transcript().entries.is_empty());
    assert_eq!(harness.transcript().status, Status::Starting);
}

#[test]
fn a_running_turn_refuses_a_switch_and_an_idle_agent_takes_one() {
    let mut harness = choosing();
    let client = harness.subscribe(1, false, None);
    let turn = prompt(&mut harness, 1, "hello");
    // The choice stays open after the first prompt.
    assert!(harness.transcript().harnesses.is_some());
    let (flow, id) = set_agent(&mut harness, Agent::Codex);
    assert_eq!(flow, Flow::Continue);
    assert_eq!(rejection(&drain(&client), id), Some(Rejection::AgentFixed));
    harness.respond(turn, json!({"stopReason": "end_turn"}));
    let (flow, _) = set_agent(&mut harness, Agent::Codex);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Codex,
            harness: HarnessId(1),
            load: None,
        }
    );
}

#[test]
fn only_an_offered_agent_and_only_a_change_switch() {
    let mut harness = choosing();
    let client = harness.subscribe(1, false, None);
    let (flow, id) = set_agent(&mut harness, Agent::Gemini);
    assert_eq!(flow, Flow::Continue);
    assert_eq!(
        rejection(&drain(&client), id),
        Some(Rejection::AgentMissing)
    );
    let (flow, id) = set_agent(&mut harness, Agent::Claude);
    assert_eq!(flow, Flow::Continue);
    assert_eq!(rejection(&drain(&client), id), None);
    assert!(harness.switched.borrow().is_empty());
}

#[test]
fn prompts_waiting_for_the_old_agent_go_to_the_new_one() {
    let mut harness = harness_with(Config {
        choices: Some(vec![Agent::Claude, Agent::Codex]),
        ..config()
    });
    harness.session.start();
    harness.command(
        1,
        Command::Prompt {
            origin: None,
            text: "fix the build".to_owned(),
            attachments: Vec::new(),
        },
    );
    assert_eq!(harness.transcript().queued.len(), 1);
    set_agent(&mut harness, Agent::Codex);
    assert_eq!(harness.transcript().queued.len(), 1);
    assert!(harness.transcript().harnesses.is_some());

    let agent = launch(&mut harness);
    let initialize = requests(&agent)[0]["id"].as_i64().unwrap();
    // The first agent answering for it changes nothing.
    harness.respond(
        initialize,
        json!({"protocolVersion": 1, "agentCapabilities": {}}),
    );
    assert!(requests(&agent).is_empty());
    respond_second(
        &mut harness,
        initialize,
        json!({"protocolVersion": 1, "agentCapabilities": {}}),
    );
    let new = requests(&agent)[0]["id"].as_i64().unwrap();
    respond_second(&mut harness, new, json!({"sessionId": "c1"}));
    let sent = requests(&agent);
    assert_eq!(sent[0]["method"], "session/prompt");
    assert_eq!(sent[0]["params"]["prompt"][0]["text"], "fix the build");
    // No prompt had reached an agent, so there is nothing to hand over.
    assert_eq!(sent[0]["params"]["prompt"].as_array().unwrap().len(), 1);
    assert!(harness.transcript().harnesses.is_some());
}

#[test]
fn an_agent_that_cannot_start_leaves_the_choice_open() {
    let mut harness = choosing();
    set_agent(&mut harness, Agent::Codex);
    let error = crate::Error::AgentSpawn {
        program: "codex-acp".to_owned(),
        source: std::io::Error::from(std::io::ErrorKind::NotFound),
    };
    harness.session.launch_failed(&error);
    let Status::Failed { message } = &harness.transcript().status else {
        panic!("expected a failure");
    };
    assert!(message.starts_with("could not start the agent `codex-acp`: "));
    assert!(harness.transcript().harnesses.is_some());
    let (flow, _) = set_agent(&mut harness, Agent::Claude);
    assert_eq!(
        flow,
        Flow::Launch {
            agent: Agent::Claude,
            harness: HarnessId(2),
            load: None,
        }
    );
    assert_eq!(harness.transcript().status, Status::Starting);
}

#[test]
fn a_client_that_reconnects_after_a_switch_gets_a_snapshot() {
    let mut harness = choosing();
    let before = harness.session.cursor();
    set_agent(&mut harness, Agent::Codex);
    let client = harness.subscribe(2, false, Some(before));
    let messages = drain(&client);
    assert!(
        matches!(messages.get(1), Some(ServerMessage::Snapshot { transcript, .. })
        if transcript.agent == "Codex")
    );
}

#[test]
fn the_note_names_the_agent_switched_to() {
    let mut harness = harness_with(Config {
        choices: Some(vec![Agent::Claude, Agent::Codex]),
        prompts: crate::context::Prompts::with_instructions("You are {{ agent }}."),
        ..config()
    });
    harness.session.start();
    harness.command(
        1,
        Command::Prompt {
            origin: None,
            text: "hi".to_owned(),
            attachments: Vec::new(),
        },
    );
    set_agent(&mut harness, Agent::Codex);
    let agent = launch(&mut harness);
    let initialize = requests(&agent)[0]["id"].as_i64().unwrap();
    respond_second(
        &mut harness,
        initialize,
        json!({"protocolVersion": 1, "agentCapabilities": {}}),
    );
    let new = requests(&agent)[0]["id"].as_i64().unwrap();
    respond_second(&mut harness, new, json!({"sessionId": "c1"}));
    let sent = requests(&agent);
    let note = sent[0]["params"]["prompt"][0]["text"].as_str().unwrap();
    assert!(note.contains("You are Codex."), "{note}");
}
