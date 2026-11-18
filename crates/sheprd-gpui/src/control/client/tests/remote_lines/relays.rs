use super::*;
use sheprd_remote_control::{control::RelayState, relay_label::RelayLabel};

#[test]
fn status_says_how_each_relay_stands_in_text_and_json() {
    let mut status = status();
    status.relays.insert(
        "https://sfo3.relay.example.org".into(),
        RelayState::Listening { rtt_ms: Some(42) }.into(),
    );
    status.relays.insert(
        "https://fra1.relay.example.org".into(),
        RelayState::Failed {
            error: "the relay could not be reached".into(),
        }
        .into(),
    );
    let output = remote::output(&Reply::Status {
        enabled: true,
        status: Some(Box::new(status)),
        error: None,
    })
    .unwrap();
    assert!(
        output
            .text
            .contains("Relay: https://sfo3.relay.example.org — listening, 42 ms")
    );
    assert!(output.text.contains(
        "Relay: https://fra1.relay.example.org — failed: the relay could not be reached"
    ));
    assert_eq!(
        output.json["status"]["relays"]["https://sfo3.relay.example.org"]["state"],
        "listening"
    );
    assert_eq!(
        output.json["status"]["relays"]["https://sfo3.relay.example.org"]["rtt_ms"],
        42
    );
}

#[test]
fn status_names_a_labelled_relay_with_its_description_and_tags() {
    let mut status = status();
    let mut fra3: sheprd_remote_control::control::RelayStatus =
        RelayState::Listening { rtt_ms: Some(42) }.into();
    fra3.label = RelayLabel {
        description: Some("not for public use".into()),
        name: Some("fra3".into()),
        tags: vec!["eu".into(), "private".into()],
    };
    status
        .relays
        .insert("https://fra3.example.org".into(), fra3);
    let output = remote::output(&Reply::Status {
        enabled: true,
        status: Some(Box::new(status)),
        error: None,
    })
    .unwrap();
    assert!(output.text.ends_with(
        "Relay: fra3 (https://fra3.example.org) — listening, 42 ms\n  not for public use\n  Tags: eu, private"
    ));
    let relay = &output.json["status"]["relays"]["https://fra3.example.org"];
    assert_eq!(relay["state"], "listening");
    assert_eq!(relay["rtt_ms"], 42);
    assert_eq!(relay["name"], "fra3");
    assert_eq!(relay["description"], "not for public use");
    assert_eq!(relay["tags"], serde_json::json!(["eu", "private"]));
}
