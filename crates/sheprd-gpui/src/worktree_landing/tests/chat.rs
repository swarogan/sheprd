//! A worktree that opens as a chat: the chat row's tiles, choosing across
//! both rows, and remembering a chat.

use super::*;

fn picker_with(chosen: Landing, chats: bool) -> Picker {
    Picker::new(
        crate::teleport::host_for(&sheprd_client::ConnectTarget::Local).unwrap(),
        chosen,
        chats,
    )
}

#[test]
fn chats_are_offered_for_installed_agents_herdr_acp_runs() {
    let mut picker = picker_with(Landing::Terminal, true);
    picker.offer(Ok(vec![
        AgentKind::Claude,
        AgentKind::Codex,
        AgentKind::Droid,
    ]));
    // Droid has no ACP adapter, so it has a terminal tile and no chat tile.
    assert!(picker.offered().contains(&Landing::Agent(AgentKind::Droid)));
    assert_eq!(
        picker.offered_chats(),
        [Landing::Chat(Agent::Claude), Landing::Chat(Agent::Codex)]
    );
}

#[test]
fn one_choice_spans_both_rows() {
    let mut picker = picker_with(Landing::Agent(AgentKind::Claude), true);
    picker.offer(Ok(vec![AgentKind::Claude, AgentKind::Codex]));
    assert!(picker.choose(Landing::Chat(Agent::Codex)));
    assert_eq!(picker.chosen, Landing::Chat(Agent::Codex));
    assert!(picker.choose(Landing::Agent(AgentKind::Claude)));
    assert_eq!(picker.chosen, Landing::Agent(AgentKind::Claude));
    // An agent the host lacks has no chat to choose.
    assert!(!picker.choose(Landing::Chat(Agent::Gemini)));
    assert_eq!(picker.chosen, Landing::Agent(AgentKind::Claude));
}

#[test]
fn no_chat_row_where_no_chat_can_start() {
    let mut picker = picker_with(Landing::Chat(Agent::Claude), false);
    // The remembered chat lands in its agent's terminal instead.
    assert_eq!(picker.chosen, Landing::Agent(AgentKind::Claude));
    picker.offer(Ok(vec![AgentKind::Claude]));
    assert!(picker.offered_chats().is_empty());
    assert!(!picker.choose(Landing::Chat(Agent::Claude)));
}

#[test]
fn a_remembered_chat_shows_while_looking_and_falls_back_when_missing() {
    let mut picker = picker_with(Landing::Chat(Agent::Claude), true);
    assert!(picker.looking());
    assert_eq!(picker.offered_chats(), [Landing::Chat(Agent::Claude)]);
    picker.offer(Ok(vec![AgentKind::Codex]));
    assert_eq!(picker.chosen, Landing::Terminal);
    assert_eq!(picker.offered_chats(), [Landing::Chat(Agent::Codex)]);
}

#[test]
fn a_chat_is_remembered_apart_from_its_agent_in_the_terminal() {
    let saved = serde_json::to_vec(&Saved::from(Landing::Chat(Agent::Opencode))).unwrap();
    assert_eq!(saved, br#"{"agent":"opencode","chat":true}"#);
    assert_eq!(parse(&saved).unwrap(), Landing::Chat(Agent::Opencode));
    // A terminal agent's file reads as older versions wrote it.
    let saved = serde_json::to_vec(&Saved::from(Landing::Agent(AgentKind::Opencode))).unwrap();
    assert_eq!(saved, br#"{"agent":"opencode"}"#);
    assert_eq!(
        parse(br#"{"agent":"someday-agent","chat":true}"#).unwrap(),
        Landing::Terminal
    );
}

#[test]
fn a_chat_that_can_no_longer_start_falls_back_while_the_dialog_is_open() {
    let mut picker = picker_with(Landing::Chat(Agent::Claude), true);
    picker.offer(Ok(vec![AgentKind::Claude]));
    assert_eq!(picker.chosen, Landing::Chat(Agent::Claude));
    // As when the search for herdr-acp ends without finding it.
    picker.allow_chats(false);
    assert_eq!(picker.chosen, Landing::Agent(AgentKind::Claude));
    assert!(picker.offered_chats().is_empty());
    picker.allow_chats(true);
    assert_eq!(picker.offered_chats(), [Landing::Chat(Agent::Claude)]);
}

#[test]
fn a_chat_whose_agent_the_host_lacks_falls_back_to_the_shell() {
    let mut picker = picker_with(Landing::Chat(Agent::Copilot), true);
    picker.offer(Err(sheprd_client::Error::Disconnected));
    // Every agent is offered when the host could not be looked at.
    assert_eq!(picker.chosen, Landing::Chat(Agent::Copilot));
    picker.installed = Some(vec![AgentKind::Claude]);
    picker.allow_chats(false);
    assert_eq!(picker.chosen, Landing::Terminal);
}
