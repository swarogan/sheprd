//! `usage pick`: the agent the Agent button would start a chat with now,
//! and the plan room of every agent installed on the selected host, as the
//! status bar last read it.

use std::time::SystemTime;

use sheprd_chat_protocol::Agent;
use sheprd_usage::{model::countdown, pick::Room};

use crate::{
    SheprdWindow,
    control::protocol::{
        Response,
        hosts::{AgentRoom, Reply, RoomState, UsagePick},
    },
};

pub(super) fn pick(view: &SheprdWindow, now: SystemTime) -> Response {
    let host = super::selected_label(view);
    let agents = view.chat_agents();
    let rooms = view.chat_agent_rooms(&agents, now);
    let pick = view.suggested_chat_agent(&agents, now);
    let note = if !view.can_start_chat() {
        Some(format!("Chats cannot start on {host} now"))
    } else if agents.is_empty() {
        Some(format!("No chat agent is installed on {host}"))
    } else if !view.config.usage.show {
        Some(
            "Usage is turned off ([usage] show in the settings), so the agent last used is kept"
                .to_owned(),
        )
    } else {
        None
    };
    let instead_of = pick.and_then(|pick| pick.instead_of);
    let pick = UsagePick {
        host,
        agent: pick.map(|pick| pick.chosen.name().to_owned()),
        instead_of: instead_of.map(|(agent, _)| agent.name().to_owned()),
        reason: instead_of.and_then(|(agent, room)| reason(agent, &room)),
        candidates: rooms
            .iter()
            .map(|(agent, room)| agent_room(*agent, room))
            .collect(),
        note,
    };
    Response::Host {
        reply: Reply::UsagePick(Box::new(pick)),
    }
}

fn agent_room(agent: Agent, room: &Room) -> AgentRoom {
    let seconds = |left: Option<std::time::Duration>| left.map(|left| left.as_secs());
    let (state, used, resets_in, runs_out) = match *room {
        Room::Ample { used } => (RoomState::Ample, Some(used), None, None),
        Room::Unknown => (RoomState::Unknown, None, None, None),
        Room::Tight {
            used,
            resets_in,
            runs_out,
        } => (RoomState::Tight, Some(used), resets_in, runs_out),
        Room::Spent { resets_in } => (RoomState::Spent, Some(100.), resets_in, None),
    };
    AgentRoom {
        agent: agent.name().to_owned(),
        room: state,
        used_percent: used,
        resets_in_seconds: seconds(resets_in),
        runs_out_in_seconds: seconds(runs_out),
    }
}

/// Why `agent` was passed over, in English as the command line speaks.
fn reason(agent: Agent, room: &Room) -> Option<String> {
    let agent = agent.display_name();
    Some(match *room {
        Room::Spent {
            resets_in: Some(left),
        } => format!(
            "{agent} has hit its limit until it resets in {}",
            countdown(left)
        ),
        Room::Spent { resets_in: None } => format!("{agent} has hit its limit"),
        Room::Tight {
            used,
            runs_out: Some(left),
            ..
        } => format!(
            "{agent} is {used:.0}% used and runs out in {} at this pace",
            countdown(left)
        ),
        Room::Tight {
            used,
            resets_in: Some(left),
            ..
        } => format!("{agent} is {used:.0}% used, resets in {}", countdown(left)),
        Room::Tight { used, .. } => format!("{agent} is {used:.0}% used"),
        Room::Ample { .. } | Room::Unknown => return None,
    })
}

#[cfg(test)]
mod tests;
