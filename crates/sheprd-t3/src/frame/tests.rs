use super::*;

fn recorded(name: &str) -> Vec<Value> {
    crate::fixtures::frames(match name {
        "snapshots" => crate::fixtures::SNAPSHOTS,
        _ => crate::fixtures::LIVE,
    })
}

#[test]
fn every_recorded_server_frame_decodes() {
    for name in ["snapshots", "live"] {
        for frame in recorded(name) {
            let decoded: ServerFrame = serde_json::from_value(frame.clone()).unwrap();
            assert_ne!(decoded, ServerFrame::Unknown, "{frame}");
        }
    }
}

#[test]
fn a_recorded_call_succeeds_with_its_value() {
    let exit = recorded("live")
        .into_iter()
        .filter_map(|frame| serde_json::from_value::<ServerFrame>(frame).ok())
        .find_map(|frame| match frame {
            ServerFrame::Exit { request_id, exit } if request_id == "3" => Some(exit),
            _ => None,
        })
        .unwrap();
    let Exit::Success { value } = exit else {
        panic!("the dispatch succeeded");
    };
    assert!(value["sequence"].as_u64().is_some());
}

#[test]
fn a_failure_names_its_cause() {
    // As 0.1.7 refused a dispatch missing a field.
    let frame: ServerFrame = serde_json::from_str(
        r#"{"_tag":"Exit","requestId":"3","exit":{"_tag":"Failure",
            "cause":[{"_tag":"Die","defect":"Missing key\n  at [\"dispatchMode\"]"}]}}"#,
    )
    .unwrap();
    let ServerFrame::Exit { exit, .. } = frame else {
        panic!("an exit");
    };
    assert_eq!(
        exit.cause().as_deref(),
        Some("Missing key\n  at [\"dispatchMode\"]")
    );
    assert_eq!(Exit::Success { value: Value::Null }.cause(), None);
}

#[test]
fn frames_of_a_newer_kind_are_kept_as_unknown() {
    let frame: ServerFrame =
        serde_json::from_str(r#"{"_tag":"ClientProtocolError","error":"x"}"#).unwrap();
    assert_eq!(frame, ServerFrame::Unknown);
    let frame: ServerFrame =
        serde_json::from_str(r#"{"_tag":"Exit","requestId":"1","exit":{"_tag":"Interrupt"}}"#)
            .unwrap();
    assert!(matches!(
        frame,
        ServerFrame::Exit {
            exit: Exit::Unknown,
            ..
        }
    ));
}

#[test]
fn client_frames_match_what_t3s_own_client_sends() {
    let request = ClientFrame::Request {
        id: "1".to_owned(),
        tag: "orchestration.subscribeShell".to_owned(),
        payload: serde_json::json!({}),
        headers: [],
    };
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        serde_json::json!({"_tag": "Request", "id": "1",
            "tag": "orchestration.subscribeShell", "payload": {}, "headers": []})
    );
    let ack = ClientFrame::Ack {
        request_id: "1".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&ack).unwrap(),
        serde_json::json!({"_tag": "Ack", "requestId": "1"})
    );
}
