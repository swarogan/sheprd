//! A new worktree that opens as a chat: the line typed into its root pane,
//! which carries the program and `[chat.env]` itself, and the ways it is
//! refused.

use crate::{
    chat::start::{self, AgentChoice, shell_line},
    sidebar::layout_tests::fixture_window,
    window::MockPeer,
    worktree_landing::{Landing, Launch, Picker},
};
use sheprd_chat_protocol::Agent;
use sheprd_client::protocol::{ClientMessage as DaemonMessage, ClientPaneInputEvent};
use std::{collections::BTreeMap, path::Path};

#[test]
fn the_line_carries_the_program_its_choice_and_environment() {
    let program = Path::new("/Applications/Sheprd.app/Contents/MacOS/herdr-acp");
    assert_eq!(
        shell_line(Agent::Claude, program, AgentChoice::Fixed, &BTreeMap::new()).unwrap(),
        " sh -c 'exec \"$0\" \"$@\"' '/Applications/Sheprd.app/Contents/MacOS/herdr-acp' 'claude'"
    );
    let env = BTreeMap::from([("ANTHROPIC_MODEL".to_owned(), "opus".to_owned())]);
    assert_eq!(
        shell_line(Agent::Codex, program, AgentChoice::Open, &env).unwrap(),
        " sh -c 'exec \"$0\" \"$@\"' 'env' 'ANTHROPIC_MODEL=opus' \
         '/Applications/Sheprd.app/Contents/MacOS/herdr-acp' '--choose' 'codex'"
    );
    // A quote would end the word early, so no line is typed.
    let env = BTreeMap::from([("NOTE".to_owned(), "it's".to_owned())]);
    assert!(shell_line(Agent::Claude, program, AgentChoice::Fixed, &env).is_none());
}

#[gpui::test]
fn a_chat_line_needs_chats_on_and_the_program(cx: &mut gpui::TestAppContext) {
    let peer = MockPeer::advertising(&[]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            super::start::local_window(view, &peer, cx);
            assert!(view.chat_line(Agent::Claude).is_ok());
            view.chats.program = crate::chat::Program::Looking;
            assert!(view.may_start_chat());
            assert!(matches!(
                view.chat_line(Agent::Claude),
                Err(start::Error::ProgramUnknown)
            ));
            view.chats.program = crate::chat::Program::Missing;
            assert!(!view.may_start_chat());
            view.config.features.acp_chats = false;
            assert!(matches!(
                view.chat_line(Agent::Claude),
                Err(start::Error::Disabled)
            ));
        })
    });
}

/// The created checkout's root pane gets the chat's line, as a chat's own
/// tab gets its fixed one.
#[gpui::test]
fn a_created_worktree_lands_in_a_chat(cx: &mut gpui::TestAppContext) {
    let mut peer = MockPeer::advertising(&[]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            super::start::local_window(view, &peer, cx);
            let host = crate::teleport::host_for(&sheprd_client::ConnectTarget::Local).unwrap();
            let picker = Picker::new(host, Landing::Chat(Agent::Claude), true);
            let boot = view.live.snapshot.as_ref().unwrap().boot_id.clone();
            let launch = Launch::new("create", (0, 0), &boot, &picker);
            let created = serde_json::json!({"result":{"type":"worktree_created",
                "workspace":{"workspace_id":"w6"},"root_pane":{"pane_id":"w6:p1"}}});
            view.land(launch, &created, cx);
            assert!(view.local_error.is_none(), "{:?}", view.local_error);
        });
        assert_eq!(
            crate::worktree_landing::Remembered::landing(cx),
            Landing::Chat(Agent::Claude)
        );
    });
    let typed = std::iter::repeat_with(|| peer.receive())
        .take(8)
        .find(|message| matches!(message, DaemonMessage::ClientShellPaneInput { .. }));
    let line = shell_line(
        Agent::Claude,
        Path::new("/opt/herdr-acp"),
        AgentChoice::Fixed,
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        typed.unwrap(),
        DaemonMessage::ClientShellPaneInput {
            pane_id: "w6:p1".into(),
            events: vec![
                ClientPaneInputEvent::TextCommit(line),
                crate::menu::enter_key(),
            ],
        }
    );
}

/// The search for the program ends while the dialog is open: a chat it
/// can no longer start is not left chosen.
#[gpui::test]
fn the_open_dialog_follows_the_program_search(cx: &mut gpui::TestAppContext) {
    let peer = MockPeer::advertising(&[]);
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            super::start::local_window(view, &peer, cx);
            view.chats.program = crate::chat::Program::Looking;
            let host = crate::teleport::host_for(&sheprd_client::ConnectTarget::Local).unwrap();
            let mut picker = Picker::new(host, Landing::Chat(Agent::Codex), view.may_start_chat());
            picker.offer(Ok(vec![crate::teleport::AgentKind::Codex]));
            view.menu.landing = Some(picker);
            view.chats.program = crate::chat::Program::Missing;
            view.refresh_landing_chats();
            let picker = view.menu.landing.as_ref().unwrap();
            assert_eq!(
                picker.chosen,
                Landing::Agent(crate::teleport::AgentKind::Codex)
            );
            assert!(picker.offered_chats().is_empty());
        })
    });
}
