use super::*;
use sheprd_remote_control::control::RelayState;

#[test]
fn status_says_how_each_relay_stands_in_text_and_json() {
    let mut status = status();
    status.relays.insert(
        "https://sfo3.relay.example.org".into(),
        RelayState::Listening,
    );
    status.relays.insert(
        "https://fra1.relay.example.org".into(),
        RelayState::Failed {
            error: "the relay could not be reached".into(),
        },
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
            .contains("Relay: https://sfo3.relay.example.org — listening")
    );
    assert!(output.text.contains(
        "Relay: https://fra1.relay.example.org — failed: the relay could not be reached"
    ));
    assert_eq!(
        output.json["status"]["relays"]["https://sfo3.relay.example.org"]["state"],
        "listening"
    );
}
