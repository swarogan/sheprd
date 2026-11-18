#![allow(clippy::unwrap_used)]

use std::time::Duration;

use super::*;

#[test]
fn each_room_reads_as_its_state_and_numbers() {
    let tight = Room::Tight {
        used: 87.,
        resets_in: Some(Duration::from_secs(7800)),
        runs_out: Some(Duration::from_secs(2400)),
    };
    assert_eq!(
        agent_room(Agent::Claude, &tight),
        AgentRoom {
            agent: "claude".into(),
            room: RoomState::Tight,
            used_percent: Some(87.),
            resets_in_seconds: Some(7800),
            runs_out_in_seconds: Some(2400),
        }
    );
    let unknown = agent_room(Agent::Gemini, &Room::Unknown);
    assert_eq!(unknown.room, RoomState::Unknown);
    assert_eq!(unknown.used_percent, None);
    let spent = agent_room(Agent::Codex, &Room::Spent { resets_in: None });
    assert_eq!(
        (spent.room, spent.used_percent),
        (RoomState::Spent, Some(100.))
    );
}

#[test]
fn the_reason_is_english_and_only_for_short_plans() {
    let running_out = Room::Tight {
        used: 66.,
        resets_in: Some(Duration::from_secs(4 * 3600)),
        runs_out: Some(Duration::from_secs(40 * 60)),
    };
    assert_eq!(
        reason(Agent::Claude, &running_out).unwrap(),
        "Claude is 66% used and runs out in 40m at this pace"
    );
    let spent = Room::Spent {
        resets_in: Some(Duration::from_secs(3600)),
    };
    assert_eq!(
        reason(Agent::Codex, &spent).unwrap(),
        "Codex has hit its limit until it resets in 1h 0m"
    );
    assert_eq!(reason(Agent::Codex, &Room::Ample { used: 5. }), None);
    assert_eq!(reason(Agent::Codex, &Room::Unknown), None);
}
