use serde_json::json;

use super::*;

#[test]
fn a_prompt_carries_its_origin_and_reads_without_one() {
    let prompt = Command::Prompt {
        text: "hi".to_owned(),
        attachments: Vec::new(),
        origin: Some(Origin::new(Device::PhoneApp, Via::Relay)),
    };
    let value = serde_json::to_value(&prompt).unwrap();
    assert_eq!(
        value,
        json!({
            "command": "prompt", "text": "hi", "attachments": [],
            "origin": {"device": "phone_app", "via": "relay"},
        })
    );
    assert_eq!(serde_json::from_value::<Command>(value).unwrap(), prompt);

    let older = json!({"command": "prompt", "text": "hi"});
    let Command::Prompt { origin, .. } = serde_json::from_value(older).unwrap() else {
        panic!("not a prompt");
    };
    assert_eq!(origin, None);
}

#[test]
fn an_origin_from_a_newer_client_still_reads() {
    let origin: Origin =
        serde_json::from_value(json!({"device": "watch", "via": "satellite"})).unwrap();
    assert_eq!(origin, Origin::new(Device::Other, Via::Other));
    assert!(!origin.opens_paths());
}

#[test]
fn only_this_machine_opens_its_paths() {
    assert!(Origin::new(Device::Desktop, Via::Local).opens_paths());
    assert!(Origin::new(Device::Terminal, Via::Local).opens_paths());
    assert!(!Origin::new(Device::Desktop, Via::Remote).opens_paths());
    assert!(!Origin::new(Device::PhoneApp, Via::Tailscale).opens_paths());
    assert!(!Origin::new(Device::PhoneWeb, Via::Local).opens_paths());
}
