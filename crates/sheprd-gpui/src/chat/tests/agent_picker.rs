//! A chat that may still switch agents offers them in the composer, and
//! stops offering them once its first prompt reached the agent.

use super::{
    overlay::{attach_with, open, poll},
    *,
};
use gpui::{Modifiers, TestAppContext};
use sheprd_chat_protocol::{Agent, Command, Harnesses};
use sheprd_client::ConnectTarget;

fn next_command(stream: &mut Peer) -> Command {
    match read(stream) {
        Some(ClientMessage::Command { command, .. }) => command,
        other => panic!("a command, got {other:?}"),
    }
}

fn choosing(epoch: u128, agent: Agent, available: &[Agent]) -> ServerMessage {
    let mut transcript = Transcript::new(agent.display_name());
    transcript.status = Status::Idle;
    transcript.harnesses = Some(Harnesses {
        available: available.to_vec(),
        current: Some(agent),
    });
    ServerMessage::Snapshot {
        cursor: cursor(epoch, 0),
        transcript: Box::new(transcript),
    }
}

#[gpui::test]
fn the_picker_switches_the_agent_and_goes_with_the_first_prompt(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let both = [Agent::Claude, Agent::Codex];
    let mut stream = attach_with(&view, &program, choosing(1, Agent::Claude, &both), cx);
    let picker = cx.debug_bounds("chat-agent-picker").unwrap();
    cx.simulate_click(picker.center(), Modifiers::default());
    poll(&view, &program, cx);
    assert!(cx.debug_bounds("chat-menu-row-0").is_some());
    let codex = cx.debug_bounds("chat-menu-row-1").unwrap();
    cx.simulate_click(codex.center(), Modifiers::default());
    poll(&view, &program, cx);
    assert!(cx.debug_bounds("chat-menu").is_none());
    assert_eq!(
        next_command(&mut stream),
        Command::SetAgent {
            agent: Agent::Codex
        }
    );
    // The next chat the launcher starts uses the agent picked here.
    let last = view.read_with(cx, |view, _| view.chats.last_started);
    assert_eq!(last, Some(Agent::Codex));

    // The program starts over with the new agent, still offering both.
    send(&mut stream, &choosing(2, Agent::Codex, &both));
    wait_until(|| {
        poll(&view, &program, cx);
        view.read_with(cx, |view, _| {
            view.chats
                .panes()
                .any(|pane| pane.session.transcript().agent == "Codex")
        })
    });
    assert!(cx.debug_bounds("chat-agent-picker").is_some());

    // The first prompt closes the choice.
    send(
        &mut stream,
        &ServerMessage::Change {
            change: Change::Harnesses { harnesses: None },
            cursor: cursor(2, 1),
        },
    );
    wait_until(|| {
        poll(&view, &program, cx);
        cx.debug_bounds("chat-agent-picker").is_none()
    });
}

#[gpui::test]
fn picking_the_running_agent_sends_nothing(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let both = [Agent::Claude, Agent::Codex];
    let mut stream = attach_with(&view, &program, choosing(1, Agent::Claude, &both), cx);
    let picker = cx.debug_bounds("chat-agent-picker").unwrap();
    cx.simulate_click(picker.center(), Modifiers::default());
    poll(&view, &program, cx);
    let claude = cx.debug_bounds("chat-menu-row-0").unwrap();
    cx.simulate_click(claude.center(), Modifiers::default());
    poll(&view, &program, cx);
    cx.simulate_input("hello");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        next_command(&mut stream),
        Command::Prompt {
            origin: Some(TYPED_HERE),
            text: "hello".into(),
            attachments: Vec::new(),
        }
    );
}

#[gpui::test]
fn a_lone_agent_offers_no_picker(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let lone = choosing(1, Agent::Claude, &[Agent::Claude]);
    let _stream = attach_with(&view, &program, lone, cx);
    assert!(cx.debug_bounds("chat-agent-picker").is_none());
}

#[gpui::test]
fn a_fixed_chat_offers_no_picker(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let fixed = snapshot(cursor(1, 0), Vec::new());
    let _stream = attach_with(&view, &program, fixed, cx);
    assert!(cx.debug_bounds("chat-agent-picker").is_none());
}
