//! Each question with choices goes to clients whole, and their own answers,
//! one or several, come back as Claude's input.

use super::turns::{can_use_tool, chosen, permission_request};
use super::*;

fn answered(id: i64, answers: Value) -> Inbound {
    Inbound::Response {
        id: wire::RequestId::Number(id),
        result: Ok(json!({"outcome": {
            "outcome": "selected",
            "optionId": "herdr.answers",
            "_meta": {"herdr.answers": answers},
        }})),
    }
}

fn ask(bridge: &mut Bridge, multiple: bool) -> (i64, Value) {
    prompt(bridge, 2, "ask me");
    let input = json!({"questions": [{
        "question": "Which details should a row show?",
        "header": "Row details",
        "multiSelect": multiple,
        "options": [
            {"label": "Running command", "description": "The foreground process."},
            {"label": "Git branch"},
        ],
    }]});
    let outs = bridge.on_harness(&can_use_tool("ctl_q", "AskUserQuestion", input, json!([])));
    let (id, params) = permission_request(&outs[0]);
    (id, params.clone())
}

#[test]
fn the_question_goes_to_clients_whole_with_descriptions() {
    let mut bridge = opened();
    let (_, params) = ask(&mut bridge, true);
    assert_eq!(
        params["_meta"]["herdr.question"],
        json!({
            "header": "Row details",
            "multiple": true,
            "options": [
                {"description": "The foreground process.", "label": "Running command"},
                {"label": "Git branch"},
            ],
            "text": "Which details should a row show?",
        })
    );
    // Clients that do not know the question pick one choice, or answer in
    // the chat.
    let names: Vec<&str> = params["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Running command", "Git branch", "Answer in the chat"]
    );
}

#[test]
fn answers_to_one_of_several_questions_ask_the_next() {
    let mut bridge = opened();
    prompt(&mut bridge, 2, "ask me");
    let input = json!({"questions": [
        {"question": "Which database?", "options": [{"label": "Postgres"}]},
        {"question": "Which features?", "multiSelect": true, "options": [{"label": "Search"}]},
    ]});
    let outs = bridge.on_harness(&can_use_tool("ctl_q", "AskUserQuestion", input, json!([])));
    let (first, params) = permission_request(&outs[0]);
    assert_eq!(params["_meta"]["herdr.question"]["text"], "Which database?");
    let outs = bridge.on_session(answered(first, json!(["MySQL"])));
    let (second, params) = permission_request(&outs[0]);
    assert_eq!(params["_meta"]["herdr.question"]["text"], "Which features?");
    assert_eq!(params["_meta"]["herdr.question"]["multiple"], true);
    let answer = bridge.on_session(answered(second, json!(["Search", "offline mode"])));
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(
        decision["updatedInput"]["answers"],
        json!({"Which database?": "MySQL", "Which features?": "Search, offline mode"})
    );
}

#[test]
fn several_answers_reach_claude_joined() {
    let mut bridge = opened();
    let (id, _) = ask(&mut bridge, true);
    let answer = bridge.on_session(answered(id, json!(["Running command", "exit codes"])));
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(decision["behavior"], "allow");
    assert_eq!(
        decision["updatedInput"]["answers"]["Which details should a row show?"],
        "Running command, exit codes"
    );
}

#[test]
fn a_single_choice_question_still_takes_its_options() {
    let mut bridge = opened();
    let (id, _) = ask(&mut bridge, false);
    let answer = bridge.on_session(chosen(id, "answer-1"));
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(
        decision["updatedInput"]["answers"]["Which details should a row show?"],
        "Git branch"
    );
}

#[test]
fn answers_without_text_cancel_the_question() {
    let mut bridge = opened();
    let (id, _) = ask(&mut bridge, true);
    let answer = bridge.on_session(answered(id, json!([])));
    let decision = &claude(&answer[0])["response"]["response"];
    assert_eq!(decision["behavior"], "deny");
    assert_eq!(decision["interrupt"], true);
}

#[test]
fn other_permissions_carry_no_question() {
    let mut bridge = opened();
    prompt(&mut bridge, 2, "run it");
    let input = json!({"command": "cargo test"});
    let outs = bridge.on_harness(&can_use_tool("ctl_b", "Bash", input, json!([])));
    let (id, params) = permission_request(&outs[0]);
    assert!(params.get("_meta").is_none());
    // Answers sent anyway are not an option of the request.
    let answer = bridge.on_session(answered(id, json!(["yes"])));
    assert_eq!(
        claude(&answer[0])["response"]["response"]["behavior"],
        "deny"
    );
}
