use serde_json::json;

use super::*;

fn offered(current: Option<Agent>) -> Harnesses {
    Harnesses {
        available: vec![Agent::Claude, Agent::Codex, Agent::Pi],
        current,
    }
}

#[test]
fn agents_travel_by_name() {
    assert_eq!(serde_json::to_value(Agent::Codex).unwrap(), json!("codex"));
    assert_eq!(
        serde_json::from_value::<Agent>(json!("opencode")).unwrap(),
        Agent::Opencode
    );
    assert!(serde_json::from_value::<Agent>(json!("nope")).is_err());
}

#[test]
fn agents_a_newer_program_offers_are_left_out_not_fatal() {
    let harnesses: Harnesses = serde_json::from_value(json!({
        "available": ["claude", "future-agent", "codex"],
        "current": "future-agent",
    }))
    .unwrap();
    assert_eq!(
        harnesses,
        Harnesses {
            available: vec![Agent::Claude, Agent::Codex],
            current: None,
        }
    );
}

#[test]
fn harnesses_round_trip_and_stay_out_of_fixed_chats() {
    let mut transcript = Transcript::new("Claude");
    let fixed = serde_json::to_value(&transcript).unwrap();
    assert!(fixed.get("harnesses").is_none());
    transcript.harnesses = Some(offered(Some(Agent::Claude)));
    let value = serde_json::to_value(&transcript).unwrap();
    assert_eq!(
        value["harnesses"],
        json!({"available": ["claude", "codex", "pi"], "current": "claude"})
    );
    assert_eq!(
        serde_json::from_value::<Transcript>(value).unwrap(),
        transcript
    );
}

#[test]
fn the_change_opens_and_closes_the_choice() {
    let mut transcript = Transcript::new("Claude");
    let open = Change::Harnesses {
        harnesses: Some(offered(Some(Agent::Claude))),
    };
    transcript.apply(&open).unwrap();
    assert_eq!(transcript.harnesses, Some(offered(Some(Agent::Claude))));
    transcript
        .apply(&Change::Harnesses { harnesses: None })
        .unwrap();
    assert_eq!(transcript.harnesses, None);
}

#[test]
fn stepping_wraps_and_needs_an_alternative() {
    let harnesses = offered(Some(Agent::Claude));
    assert_eq!(harnesses.step(1), Some(Agent::Codex));
    assert_eq!(harnesses.step(-1), Some(Agent::Pi));
    assert_eq!(offered(Some(Agent::Pi)).step(1), Some(Agent::Claude));
    // An agent no longer offered starts from the first.
    assert_eq!(offered(Some(Agent::Gemini)).step(1), Some(Agent::Claude));
    let alone = Harnesses {
        available: vec![Agent::Claude],
        current: Some(Agent::Claude),
    };
    assert_eq!(alone.step(1), None);
}

#[test]
fn step_agent_asks_only_while_the_choice_is_open() {
    let mut transcript = Transcript::new("Claude");
    assert_eq!(crate::assist::step_agent(&transcript, 1), None);
    transcript.harnesses = Some(offered(Some(Agent::Claude)));
    assert_eq!(
        crate::assist::step_agent(&transcript, 1),
        Some(Command::SetAgent {
            agent: Agent::Codex
        })
    );
}

#[test]
fn set_agent_and_its_rejections_have_stable_names() {
    let command = Command::SetAgent { agent: Agent::Pi };
    assert_eq!(
        serde_json::to_value(&command).unwrap(),
        json!({"command": "set_agent", "agent": "pi"})
    );
    assert_eq!(
        serde_json::to_value(Rejection::AgentFixed).unwrap(),
        json!("agent_fixed")
    );
    assert_eq!(
        serde_json::to_value(Rejection::AgentMissing).unwrap(),
        json!("agent_missing")
    );
}
