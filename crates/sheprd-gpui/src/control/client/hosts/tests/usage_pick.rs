use super::*;
use crate::control::protocol::hosts::{AgentRoom, RoomState, UsagePick};

fn room(agent: &str, room: RoomState, used: Option<f32>) -> AgentRoom {
    AgentRoom {
        agent: agent.into(),
        room,
        used_percent: used,
        resets_in_seconds: Some(7800),
        runs_out_in_seconds: None,
    }
}

#[test]
fn the_pick_reads_its_reason_then_each_agents_room() {
    let pick = UsagePick {
        host: "Local".into(),
        agent: Some("codex".into()),
        instead_of: Some("claude".into()),
        reason: Some("Claude is 87% used, resets in 2h 10m".into()),
        candidates: vec![
            room("claude", RoomState::Tight, Some(87.)),
            room("codex", RoomState::Ample, Some(10.)),
            AgentRoom {
                resets_in_seconds: None,
                ..room("gemini", RoomState::Unknown, None)
            },
        ],
        note: None,
    };
    assert_eq!(
        text::usage_pick(&pick),
        "codex: Claude is 87% used, resets in 2h 10m\n  \
         claude     tight     87% used, resets in 2h 10m\n  \
         codex      ample     10% used, resets in 2h 10m\n  \
         gemini     unknown"
    );
}

#[test]
fn no_agent_says_where_and_why() {
    let pick = UsagePick {
        host: "devbox".into(),
        agent: None,
        instead_of: None,
        reason: None,
        candidates: Vec::new(),
        note: Some("No chat agent is installed on devbox".into()),
    };
    assert_eq!(
        text::usage_pick(&pick),
        "No agent to start on devbox\nNo chat agent is installed on devbox"
    );
}

#[test]
fn usage_help_describes_pick() {
    let help = super::super::super::help::usage(Some("usage"));
    assert!(help.contains("sheprd usage pick"), "{help}");
}
