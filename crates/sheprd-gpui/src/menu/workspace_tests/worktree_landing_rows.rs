//! The new worktree dialog's two landing rows: in a terminal, then as a
//! chat, with their tiles starting at one edge inside the dialog.

use super::*;
use crate::{
    teleport::AgentKind,
    worktree_landing::{Landing, Remembered},
};
use sheprd_chat_protocol::Agent;

#[gpui::test]
fn the_chat_row_sits_under_the_terminal_row_on_its_edge(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(sidebar::layout_tests::fixture_window);
    cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(700.)));
    cx.update(|window, cx| {
        Remembered::record(Landing::Chat(Agent::Codex), cx);
        view.update(cx, |view, cx| {
            let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
            snapshot.workspaces = sidebar::layout_tests::snapshot(7).workspaces;
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.config.features.acp_chats = true;
            view.endpoints[0].connection.target = sheprd_client::ConnectTarget::Local;
            view.live.local_daemon_peer = true;
            view.endpoints[0].live = view.live.clone();
            view.chats.program = crate::chat::Program::Found("/opt/herdr-acp".into());
            view.open_workspace_menu("w3", Default::default(), window, cx);
            view.open_workspace_dialog(WorkspaceAction::NewWorktree, window, cx);
            let picker = view.menu.landing.as_mut().unwrap();
            picker.offer(Ok(vec![
                AgentKind::Claude,
                AgentKind::Codex,
                AgentKind::Droid,
            ]));
            assert_eq!(picker.chosen, Landing::Chat(Agent::Codex));
        });
        window.draw(cx).clear(cx);
    });
    let panel = cx.debug_bounds("menu-panel").unwrap();
    let terminal = cx.debug_bounds("worktree-landing-terminal-row").unwrap();
    let chat = cx.debug_bounds("worktree-landing-chat-row").unwrap();
    assert!(terminal.bottom() <= chat.top(), "{terminal:?} {chat:?}");
    let shell = cx.debug_bounds("worktree-landing-terminal").unwrap();
    let droid = cx.debug_bounds("worktree-landing-droid").unwrap();
    let first_chat = cx.debug_bounds("worktree-landing-chat-claude").unwrap();
    assert_eq!(shell.left(), first_chat.left());
    // Droid runs in a terminal but has no chat.
    assert!(cx.debug_bounds("worktree-landing-chat-droid").is_none());
    for part in [terminal, chat, droid, first_chat] {
        assert!(
            panel.left() <= part.left() && part.right() <= panel.right(),
            "{part:?} outside {panel:?}"
        );
    }
}

#[gpui::test]
fn no_chat_row_while_chats_are_off(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(sidebar::layout_tests::fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
            snapshot.workspaces = sidebar::layout_tests::snapshot(7).workspaces;
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.config.features.acp_chats = false;
            view.endpoints[0].connection.target = sheprd_client::ConnectTarget::Local;
            view.open_workspace_menu("w3", Default::default(), window, cx);
            view.open_workspace_dialog(WorkspaceAction::NewWorktree, window, cx);
            let picker = view.menu.landing.as_mut().unwrap();
            picker.offer(Ok(vec![AgentKind::Claude]));
        });
        window.draw(cx).clear(cx);
    });
    assert!(cx.debug_bounds("worktree-landing-terminal-row").is_some());
    assert!(cx.debug_bounds("worktree-landing-chat-row").is_none());
}
