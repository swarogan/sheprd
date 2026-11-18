use super::*;
use crate::relay_label::RelayLabel;

#[test]
fn a_relay_status_carries_its_label_beside_its_state() {
    let status = RelayStatus {
        label: RelayLabel {
            description: Some("not for public use".into()),
            name: Some("fra3".into()),
            tags: vec!["eu".into()],
        },
        state: RelayState::Failed {
            error: "unreachable".into(),
        },
    };
    let wire = json!({
        "state": "failed", "error": "unreachable",
        "name": "fra3", "description": "not for public use", "tags": ["eu"],
    });
    assert_eq!(serde_json::to_value(&status).unwrap(), wire);
    assert_eq!(serde_json::from_value::<RelayStatus>(wire).unwrap(), status);
}

#[test]
fn an_older_daemons_relay_reads_without_a_label() {
    let older: RelayStatus = serde_json::from_value(json!({"state": "listening"})).unwrap();
    assert_eq!(older, RelayState::Listening { rtt_ms: None }.into());
    assert_eq!(
        serde_json::to_value(RelayStatus::from(RelayState::Connecting)).unwrap(),
        json!({"state": "connecting"})
    );
}

#[test]
fn an_older_app_reads_a_labelled_relays_state() {
    let state: RelayState =
        serde_json::from_value(json!({"state": "listening", "name": "fra3", "tags": ["eu"]}))
            .unwrap();
    assert_eq!(state, RelayState::Listening { rtt_ms: None });
}

#[test]
fn a_listening_relays_round_trip_sits_beside_its_label() {
    let status = RelayStatus {
        label: RelayLabel {
            name: Some("fra3".into()),
            ..RelayLabel::default()
        },
        state: RelayState::Listening { rtt_ms: Some(42) },
    };
    let wire = json!({"state": "listening", "rtt_ms": 42, "name": "fra3"});
    assert_eq!(serde_json::to_value(&status).unwrap(), wire);
    assert_eq!(serde_json::from_value::<RelayStatus>(wire).unwrap(), status);
}
