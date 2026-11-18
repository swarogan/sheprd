//! A kept workspace made for a new worktree that opens as a chat: its shell
//! is asked for before the chat, so a chat that fails, here or as it
//! starts, still leaves the checkout a tab.

use super::kept_workspaces::window;
use gpui::TestAppContext;
use sheprd_chat_protocol::Agent;
use std::path::Path;

#[gpui::test]
fn the_shell_starts_before_the_chat(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, false);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.config.features.acp_chats = true;
            view.chats.program = crate::chat::Program::Found("/opt/herdr-acp".into());
            view.new_kept_chat_workspace(Agent::Claude, Some("api"), "/src/api".into(), cx)
                .unwrap();
            let wanted: Vec<_> = view
                .core_terminals
                .wanted
                .iter()
                .map(|(_, _, spawn, _)| spawn)
                .collect();
            assert_eq!(wanted.len(), 2);
            assert!(
                wanted[0].argv.is_empty(),
                "the shell first: {:?}",
                wanted[0].argv
            );
            assert_eq!(
                wanted[1].argv.first().map(String::as_str),
                Some("/opt/herdr-acp")
            );
            for spawn in wanted {
                assert_eq!(spawn.cwd.as_deref(), Some(Path::new("/src/api")));
            }
        })
    });
}

#[gpui::test]
fn a_refused_chat_leaves_the_shell(cx: &mut TestAppContext) {
    let (view, cx) = window(cx, false);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.config.features.acp_chats = false;
            assert!(matches!(
                view.new_kept_chat_workspace(Agent::Claude, Some("api"), "/src/api".into(), cx),
                Err(crate::chat::StartError::Disabled)
            ));
            let wanted: Vec<_> = view
                .core_terminals
                .wanted
                .iter()
                .map(|(_, _, spawn, _)| spawn)
                .collect();
            assert_eq!(wanted.len(), 1);
            assert!(wanted[0].argv.is_empty());
        })
    });
}
