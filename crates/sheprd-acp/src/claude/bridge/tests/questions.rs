//! `AskUserQuestion` calls with several questions, asked one at a time.

use super::{
    turns::{can_use_tool, chosen, permission_request},
    *,
};

fn names(params: &Value) -> Vec<&str> {
    params["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["name"].as_str().unwrap())
        .collect()
}

fn two_questions() -> Value {
    json!({"questions": [
        {
            "question": "Which database?",
            "header": "Database",
            "multiSelect": false,
            "options": [{"label": "Postgres"}, {"label": "SQLite"}],
        },
        {
            "question": "Which features?",
            "header": "Features",
            "multiSelect": true,
            "options": [{"label": "Search"}, {"label": "Sync"}],
        },
    ]})
}

#[test]
fn several_questions_are_asked_in_turn_and_answered_together() {
    let mut bridge = opened();
    prompt(&mut bridge, 2, "ask me");
    let outs = bridge.on_harness(&can_use_tool(
        "ctl_q",
        "AskUserQuestion",
        two_questions(),
        json!([]),
    ));
    let (first, params) = permission_request(&outs[0]);
    assert_eq!(params["toolCall"]["title"], "Which database? (1/2)");
    assert_eq!(names(params), ["Postgres", "SQLite", "Answer in the chat"]);

    // Answering the first question asks the second; Claude hears nothing yet.
    let outs = bridge.on_session(chosen(first, "answer-0"));
    let (second, params) = permission_request(&outs[0]);
    assert_ne!(second, first);
    assert_eq!(params["toolCall"]["toolCallId"], "toolu_1");
    assert_eq!(params["toolCall"]["title"], "Which features? (2/2)");
    assert_eq!(names(params), ["Search", "Sync", "Answer in the chat"]);

    let answer = bridge.on_session(chosen(second, "answer-1"));
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(claude(&answer[0])["response"]["request_id"], "ctl_q");
    assert_eq!(decision["behavior"], "allow");
    assert_eq!(
        decision["updatedInput"]["answers"],
        json!({"Which database?": "Postgres", "Which features?": "Sync"})
    );
    assert_eq!(
        decision["updatedInput"]["questions"][1]["header"],
        "Features"
    );
}

#[test]
fn declining_a_later_question_refuses_the_whole_call() {
    let mut bridge = opened();
    prompt(&mut bridge, 2, "ask me");
    let outs = bridge.on_harness(&can_use_tool(
        "ctl_q",
        "AskUserQuestion",
        two_questions(),
        json!([]),
    ));
    let (first, _) = permission_request(&outs[0]);
    let outs = bridge.on_session(chosen(first, "answer-1"));
    let (second, _) = permission_request(&outs[0]);
    let answer = bridge.on_session(chosen(second, "reject"));
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(decision["behavior"], "deny");
    assert!(decision.get("interrupt").is_none());
}

#[test]
fn dropping_a_later_question_stops_the_turn() {
    let mut bridge = opened();
    prompt(&mut bridge, 2, "ask me");
    let outs = bridge.on_harness(&can_use_tool(
        "ctl_q",
        "AskUserQuestion",
        two_questions(),
        json!([]),
    ));
    let (first, _) = permission_request(&outs[0]);
    let outs = bridge.on_session(chosen(first, "answer-0"));
    let (second, _) = permission_request(&outs[0]);
    let answer = bridge.on_session(Inbound::Response {
        id: wire::RequestId::Number(second),
        result: Ok(json!({"outcome": {"outcome": "cancelled"}})),
    });
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(decision["behavior"], "deny");
    assert_eq!(decision["interrupt"], true);
}

#[test]
fn a_call_with_no_questions_is_left_to_the_chat() {
    let mut bridge = opened();
    prompt(&mut bridge, 2, "ask me");
    let outs = bridge.on_harness(&can_use_tool(
        "ctl_q",
        "AskUserQuestion",
        json!({"questions": []}),
        json!([]),
    ));
    let (_, params) = permission_request(&outs[0]);
    assert_eq!(names(params), ["Answer in the chat"]);
}
