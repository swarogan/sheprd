//! The composer's agent button: in a chat that may switch agents
//! (`sheprd-acp --choose`, whenever its agent is idle), it names the agent
//! running and opens a menu of the others installed where the chat runs.

use super::super::super::chats::{ChatPane, Menu};
use crate::{SheprdWindow, chat::short_text, i18n::fl, icons::AgentIcon};
use gpui::{prelude::*, *};
use sheprd_chat_protocol::{Agent, Command, Transcript};
use sheprd_usage::pick::pick;

/// The agent's mark on the button and in the menu.
const MARK: f32 = 12.;

/// The agents the chat could switch to, the running one among them, when
/// there is more than one to choose from.
pub(super) fn offered(transcript: &Transcript) -> Option<(&[Agent], Option<Agent>)> {
    let harnesses = transcript.harnesses.as_ref()?;
    (harnesses.available.len() > 1).then_some((harnesses.available.as_slice(), harnesses.current))
}

/// `agent`'s mark, found by its name as on the command line: the transcript
/// carries the display name, which names no mark.
fn mark(agent: Option<Agent>) -> AgentIcon {
    AgentIcon::from_identity(agent.map(Agent::name))
}

impl SheprdWindow {
    /// The button that opens the agent menu, while the chat offers one.
    pub(super) fn render_chat_agent_button(
        &self,
        chat: &ChatPane,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let transcript = chat.session.transcript();
        let (_, current) = offered(transcript)?;
        let name = transcript.agent.clone();
        let pane = chat.target.pane_id.clone();
        Some(
            self.chat_header_button("chat-agent-picker".into())
                .tooltip(self.chat_setting_hint(&fl!("chat-settings-agent"), &name))
                .child(mark(current).element(MARK, self.theme.foreground))
                .child(
                    div()
                        .max_w(px(160.))
                        .truncate()
                        .child(crate::notifications::safe_text(&name, 80)),
                )
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .flex_none()
                        .size(px(10.))
                        .text_color(rgb(self.theme.subtext())),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    this.toggle_chat_menu(&pane, Menu::Agent, window, cx);
                })),
        )
    }

    /// The agent menu's rows, one per agent offered, shaped by `row`.
    pub(super) fn chat_agent_rows(
        &self,
        chat: &ChatPane,
        row: impl Fn(usize, bool) -> Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some((agents, current)) = offered(chat.session.transcript()) else {
            return Vec::new();
        };
        let pane_id = chat.target.pane_id.clone();
        agents
            .iter()
            .enumerate()
            .map(|(index, &agent)| {
                let pane = pane_id.clone();
                row(index, current == Some(agent))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(mark(Some(agent)).element(MARK, self.theme.foreground))
                            .child(agent.display_name()),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        this.toggle_chat_menu(&pane, Menu::Agent, window, cx);
                        this.choose_chat_agent(&pane, agent, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }

    /// Why the chat runs another agent than the one last used, or that the
    /// one it runs is short of plan room and another is not, offering to
    /// switch: a line over the composer of a chat that may switch. Why it
    /// started with another agent matters only until the first prompt.
    pub(in crate::chat::view) fn render_chat_agent_advice(
        &self,
        chat: &ChatPane,
        size: Pixels,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let (agents, current) = offered(chat.session.transcript())?;
        let current = current?;
        let rooms = self.chat_agent_rooms(agents, std::time::SystemTime::now());
        let line = div()
            .id("chat-agent-advice")
            .debug_selector(|| "chat-agent-advice".into())
            .text_size(size)
            .text_color(rgb(self.theme.subtext()));
        let transcript = chat.session.transcript();
        // Trimming may drop every prompt of a long conversation, which a
        // truncated transcript still had.
        let fresh = !transcript.truncated
            && !transcript
                .entries
                .iter()
                .any(|entry| matches!(entry.kind, sheprd_chat_protocol::EntryKind::User { .. }));
        // The Agent button started this agent in place of the one last used.
        let launched = pick(&rooms, self.chats.last_started)?;
        if fresh
            && launched.chosen == current
            && let Some((from, room)) = launched.instead_of
            && from != current
        {
            let why = short_text(from, &room)?;
            return Some(line.child(fl!(
                "chat-pick-started-instead",
                agent = current.display_name(),
                why = why
            )));
        }
        // This chat's own agent is short: offer one with more room, judged
        // against it rather than against the agent last used elsewhere.
        let (other, (_, room)) = pick(&rooms, Some(current))
            .and_then(|switch| Some((switch.chosen, switch.instead_of?)))?;
        let why = short_text(current, &room)?;
        let pane = chat.target.pane_id.clone();
        Some(
            line.cursor_pointer()
                .hover(|line| line.text_color(rgb(self.theme.foreground)))
                .child(fl!(
                    "chat-pick-suggest",
                    why = why,
                    other = other.display_name()
                ))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.choose_chat_agent(&pane, other, cx);
                })),
        )
    }

    /// Switches the chat in `pane_id` to `agent`, which the launcher then
    /// starts the next chat with too.
    pub(in crate::chat) fn choose_chat_agent(
        &mut self,
        pane_id: &str,
        agent: Agent,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .chats
            .pane_mut(pane_id)
            .and_then(|pane| offered(pane.session.transcript()).and_then(|(_, current)| current));
        if current == Some(agent) {
            return;
        }
        self.chats.last_started = Some(agent);
        self.chat_send(pane_id, Command::SetAgent { agent }, cx);
    }
}

#[cfg(test)]
mod tests;
