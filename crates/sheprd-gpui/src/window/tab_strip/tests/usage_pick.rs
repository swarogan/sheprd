//! The Agent button starts the agent last used unless its plan is short,
//! and says why when it starts another.

use super::launcher::{AGENT, draw, strip};
use super::*;
use crate::{
    config::ChatLauncher,
    usage::{
        Host, Reading, Usage,
        model::{Account, Kind, Report, SESSION, Window},
        registry,
    },
};
use sheprd_chat_protocol::Agent;
use sheprd_usage::pick::Room;
use std::time::{Duration, SystemTime};

/// `id`'s plan, one five-hour window `used` percent spent, resetting in
/// two hours.
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

#[gpui::test]
fn a_short_plan_starts_the_agent_with_room_and_keeps_the_preference(cx: &mut TestAppContext) {
    let (view, cx, mut peer) = strip(cx, ChatLauncher::Agent);
    view.update(cx, |view, _| {
        view.chats.last_started = Some(Agent::Claude);
        view.usage = Usage::showing(
            Host::Local,
            vec![reading("claude", 92.), reading("codex", 10.)],
        );
    });
    draw(cx);
    let rooms = view.read_with(cx, |view, _| {
        view.chat_agent_rooms(&[Agent::Claude, Agent::Codex], SystemTime::now())
    });
    assert!(matches!(rooms[0], (Agent::Claude, Room::Tight { .. })));
    assert!(matches!(rooms[1], (Agent::Codex, Room::Ample { .. })));

    let agent = cx.debug_bounds(AGENT).unwrap().center();
    cx.simulate_click(agent, Modifiers::none());
    let request = peer.request();
    assert_eq!(request["method"], "tab.create");
    assert_eq!(request["params"]["label"], "Codex");
    // Usage chose Codex, not the person: the next chat prefers Claude
    // again once its plan has room.
    view.read_with(cx, |view, _| {
        assert_eq!(view.chats.last_started, Some(Agent::Claude));
    });
}

#[gpui::test]
fn a_plan_with_room_keeps_the_agent_last_used(cx: &mut TestAppContext) {
    let (view, cx, mut peer) = strip(cx, ChatLauncher::Agent);
    view.update(cx, |view, _| {
        view.chats.last_started = Some(Agent::Claude);
        view.usage = Usage::showing(
            Host::Local,
            vec![reading("claude", 60.), reading("codex", 5.)],
        );
    });
    draw(cx);
    let agent = cx.debug_bounds(AGENT).unwrap().center();
    cx.simulate_click(agent, Modifiers::none());
    assert_eq!(peer.request()["params"]["label"], "Claude");
}

#[test]
fn the_reason_names_the_agent_its_use_and_the_reset() {
    let tight = Room::Tight {
        used: 87.4,
        resets_in: Some(Duration::from_secs(2 * 3600 + 600)),
        runs_out: None,
    };
    // Fluent isolates each argument from the text around it.
    let text: String = crate::chat::short_text(Agent::Claude, &tight)
        .unwrap()
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect();
    assert_eq!(text, "Claude is 87% used, resets in 2h 10m");
    let spent = Room::Spent { resets_in: None };
    assert!(crate::chat::short_text(Agent::Codex, &spent).is_some());
    assert_eq!(
        crate::chat::short_text(Agent::Codex, &Room::Ample { used: 10. }),
        None
    );
    assert_eq!(crate::chat::short_text(Agent::Codex, &Room::Unknown), None);
}

#[gpui::test]
fn usage_turned_off_keeps_the_agent_last_used(cx: &mut TestAppContext) {
    // The last reading stays in memory, but it is no longer refreshed.
    let (view, cx, mut peer) = strip(cx, ChatLauncher::Agent);
    view.update(cx, |view, _| {
        view.chats.last_started = Some(Agent::Claude);
        view.config.usage.show = false;
        view.usage = Usage::showing(
            Host::Local,
            vec![reading("claude", 99.), reading("codex", 5.)],
        );
    });
    draw(cx);
    let agent = cx.debug_bounds(AGENT).unwrap().center();
    cx.simulate_click(agent, Modifiers::none());
    assert_eq!(peer.request()["params"]["label"], "Claude");
}
