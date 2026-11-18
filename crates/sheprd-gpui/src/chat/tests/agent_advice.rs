//! In a chat that may switch agents, a line over the composer says why it
//! started with another agent than the one last used (until its first
//! prompt), or offers one with room (at any time).

use super::{
    overlay::{attach_with, open, poll},
    *,
};
use crate::SheprdWindow;
use crate::usage::{
    Host, Reading, Usage,
    model::{Account, Kind, Report, SESSION, Window},
    registry,
};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use sheprd_chat_protocol::{Agent, Command, Harnesses};
use sheprd_client::ConnectTarget;
use std::time::SystemTime;

fn reading(id: &str, used: f64) -> Reading {
    let provider = registry::find(id).unwrap();
    let window = Window::new(
        Kind::Session,
        used,
        Some(SystemTime::now() + Duration::from_secs(2 * 3600)),
        Some(SESSION),
    );
    Reading {
        provider,
        report: Some(Report::new(provider, Account::default(), vec![window])),
        error: None,
        access: None,
    }
}

fn running(agent: Agent) -> ServerMessage {
    let mut transcript = Transcript::new(agent.display_name());
    transcript.status = Status::Idle;
    transcript.harnesses = Some(Harnesses {
        available: vec![Agent::Claude, Agent::Codex],
        current: Some(agent),
    });
    ServerMessage::Snapshot {
        cursor: cursor(1, 0),
        transcript: Box::new(transcript),
    }
}

fn claude_short(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) {
    view.update(cx, |view, _| {
        view.chats.last_started = Some(Agent::Claude);
        view.usage = Usage::showing(
            Host::Local,
            vec![reading("claude", 92.), reading("codex", 10.)],
        );
    });
}

#[gpui::test]
fn a_short_agent_offers_one_with_room(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    claude_short(&view, cx);
    let program = Program::new();
    let mut stream = attach_with(&view, &program, running(Agent::Claude), cx);
    let advice = cx.debug_bounds("chat-agent-advice").unwrap();
    cx.simulate_click(advice.center(), Modifiers::default());
    poll(&view, &program, cx);
    match read(&mut stream) {
        Some(ClientMessage::Command { command, .. }) => assert_eq!(
            command,
            Command::SetAgent {
                agent: Agent::Codex
            }
        ),
        other => panic!("a command, got {other:?}"),
    }
}

#[gpui::test]
fn a_chat_started_instead_says_why(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    claude_short(&view, cx);
    let program = Program::new();
    let _stream = attach_with(&view, &program, running(Agent::Codex), cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_some());
}

/// [`running`], once the user has said something.
fn talking(agent: Agent) -> ServerMessage {
    let ServerMessage::Snapshot {
        cursor,
        mut transcript,
    } = running(agent)
    else {
        unreachable!("running gives a snapshot");
    };
    transcript.entries.push(Entry {
        id: EntryId(1),
        kind: EntryKind::User {
            text: "hello".to_owned(),
            files: 0,
        },
    });
    ServerMessage::Snapshot { cursor, transcript }
}

#[gpui::test]
fn why_a_chat_started_instead_goes_once_the_conversation_starts(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    claude_short(&view, cx);
    let program = Program::new();
    let _stream = attach_with(&view, &program, talking(Agent::Codex), cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_none());
}

#[gpui::test]
fn why_a_chat_started_instead_stays_gone_once_its_prompts_are_trimmed(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    claude_short(&view, cx);
    let program = Program::new();
    let ServerMessage::Snapshot {
        cursor,
        mut transcript,
    } = running(Agent::Codex)
    else {
        unreachable!("running gives a snapshot");
    };
    // A long conversation whose prompts the transcript no longer holds.
    transcript.truncated = true;
    let trimmed = ServerMessage::Snapshot { cursor, transcript };
    let _stream = attach_with(&view, &program, trimmed, cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_none());
}

#[gpui::test]
fn a_short_agent_offers_one_with_room_mid_conversation(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    claude_short(&view, cx);
    let program = Program::new();
    let _stream = attach_with(&view, &program, talking(Agent::Claude), cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_some());
}

#[gpui::test]
fn plans_with_room_leave_the_composer_alone(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    view.update(cx, |view, _| {
        view.chats.last_started = Some(Agent::Claude);
        view.usage = Usage::showing(
            Host::Local,
            vec![reading("claude", 40.), reading("codex", 10.)],
        );
    });
    let program = Program::new();
    let _stream = attach_with(&view, &program, running(Agent::Claude), cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_none());
}

#[gpui::test]
fn advice_weighs_the_chats_own_agent_not_the_last_one_used(cx: &mut TestAppContext) {
    // Codex was started elsewhere since; both plans are tight, so moving
    // this Claude chat to Codex would only trade one short plan for another.
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    view.update(cx, |view, _| {
        view.chats.last_started = Some(Agent::Codex);
        view.usage = Usage::showing(
            Host::Local,
            vec![reading("claude", 85.), reading("codex", 90.)],
        );
    });
    let program = Program::new();
    let _stream = attach_with(&view, &program, running(Agent::Claude), cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_none());
}

#[gpui::test]
fn usage_turned_off_gives_no_advice(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    claude_short(&view, cx);
    view.update(cx, |view, _| view.config.usage.show = false);
    let program = Program::new();
    let _stream = attach_with(&view, &program, running(Agent::Claude), cx);
    assert!(cx.debug_bounds("chat-agent-advice").is_none());
}
