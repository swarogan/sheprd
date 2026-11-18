//! The agent a new chat starts with, by plan usage on the selected host.
//!
//! The one Agent button starts the agent the person last started or picked,
//! unless its plan is spent or close to its limit and another installed
//! agent's is not ([`sheprd_usage::pick`]). Usage is read every few minutes
//! by the status bar's worker, so this only looks at what it last read and
//! never reads anything itself.

use std::time::SystemTime;

use crate::{SheprdWindow, i18n::fl, usage};
use sheprd_chat_protocol::Agent;
use sheprd_usage::pick::{Pick, Room, pick};

impl SheprdWindow {
    /// How much room each of `agents` has on the selected host, from the
    /// usage provider of the same name, as last read. With usage turned
    /// off, nothing is known: the last reading is frozen, not current.
    pub(crate) fn chat_agent_rooms(&self, agents: &[Agent], now: SystemTime) -> Vec<(Agent, Room)> {
        let entry = if self.config.usage.show {
            self.endpoints
                .get(self.selected_endpoint)
                .and_then(|endpoint| usage::Host::of(&endpoint.connection.target))
                .and_then(|host| self.usage.entry(&host))
        } else {
            None
        };
        agents
            .iter()
            .map(|&agent| {
                let report = entry
                    .and_then(|entry| {
                        entry
                            .readings
                            .iter()
                            .find(|reading| reading.provider.id() == agent.name())
                    })
                    .and_then(|reading| reading.report.as_ref());
                (agent, Room::of(report, now))
            })
            .collect()
    }

    /// Starts the chat the Agent button suggested, `agent`, in `group`.
    /// Usage chose it, not the person: the next chat still prefers the
    /// agent they last started or picked, once its plan has room again.
    pub(crate) fn start_suggested_chat_in(
        &mut self,
        agent: Agent,
        group: crate::browser::GroupId,
        cx: &mut gpui::Context<Self>,
    ) {
        let preferred = self.chats.last_started;
        self.start_chat_in(agent, group, cx);
        if preferred.is_some() {
            self.chats.last_started = preferred;
        }
    }

    /// The agent a chat that names none starts with, among `agents`: the
    /// one last started or picked, unless its plan is short.
    pub(crate) fn suggested_chat_agent(
        &self,
        agents: &[Agent],
        now: SystemTime,
    ) -> Option<Pick<Agent>> {
        pick(&self.chat_agent_rooms(agents, now), self.chats.last_started)
    }
}

/// Why `agent`'s plan is too short to start with, in words, or `None` for
/// a plan with room or none known.
pub(crate) fn short_text(agent: Agent, room: &Room) -> Option<String> {
    let agent = agent.display_name();
    Some(match *room {
        Room::Spent {
            resets_in: Some(left),
        } => fl!(
            "chat-pick-spent-resets",
            agent = agent,
            time = usage::text::countdown(left)
        ),
        Room::Spent { resets_in: None } => fl!("chat-pick-spent", agent = agent),
        Room::Tight {
            used,
            runs_out: Some(left),
            ..
        } => fl!(
            "chat-pick-running-out",
            agent = agent,
            percent = used.round() as u32,
            time = usage::text::countdown(left)
        ),
        Room::Tight {
            used,
            resets_in: Some(left),
            ..
        } => fl!(
            "chat-pick-tight-resets",
            agent = agent,
            percent = used.round() as u32,
            time = usage::text::countdown(left)
        ),
        Room::Tight { used, .. } => fl!(
            "chat-pick-tight",
            agent = agent,
            percent = used.round() as u32
        ),
        Room::Ample { .. } | Room::Unknown => return None,
    })
}
