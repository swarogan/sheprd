//! Clients answer a question with their own choices or words.

use super::*;
use sheprd_chat_protocol::{Question, QuestionOption};

fn ask_question(harness: &mut Harness, request: i64, multiple: bool) -> PermissionId {
    harness.input(Input::Agent(HarnessId::default(), Inbound::Request {
        id: wire::RequestId::Number(request),
        method: "session/request_permission".to_owned(),
        params: json!({"sessionId": "s1",
            "toolCall": {"toolCallId": "call-q", "title": "Which details?", "kind": "other"},
            "options": [{"optionId": "reject", "name": "Answer in the chat", "kind": "reject_once"}],
            "_meta": {"herdr.question": {
                "header": "Row details",
                "multiple": multiple,
                "options": [{"label": "Running command", "description": "The process."}, {"label": "Git branch"}],
                "text": "Which details should a row show?",
            }}}),
    }));
    harness.transcript().permissions.last().unwrap().id
}

fn rejection(harness: &Harness) -> Option<Rejection> {
    harness.session.handled.back().unwrap().1
}

#[test]
fn the_question_reaches_clients_whole() {
    let mut harness = ready();
    prompt(&mut harness, 0, "test");
    ask_question(&mut harness, 9, true);
    let asked = &harness.transcript().permissions[0];
    assert_eq!(
        asked.question,
        Some(Question {
            header: Some("Row details".to_owned()),
            multiple: true,
            options: vec![
                QuestionOption {
                    description: Some("The process.".to_owned()),
                    label: "Running command".to_owned(),
                },
                QuestionOption {
                    description: None,
                    label: "Git branch".to_owned(),
                },
            ],
            text: "Which details should a row show?".to_owned(),
        })
    );
}

#[test]
fn several_answers_resolve_through_the_answers_option() {
    let mut harness = ready();
    prompt(&mut harness, 0, "test");
    let permission = ask_question(&mut harness, 9, true);
    let answers = vec!["Running command".to_owned(), "my own words".to_owned()];
    harness.command(
        0,
        Command::AnswerQuestion {
            answers,
            permission,
        },
    );
    assert_eq!(rejection(&harness), None);
    let response = harness
        .sent()
        .into_iter()
        .find(|message| message["id"] == 9)
        .unwrap();
    assert_eq!(
        response["result"]["outcome"],
        json!({"outcome": "selected", "optionId": "herdr.answers",
            "_meta": {"herdr.answers": ["Running command", "my own words"]}})
    );
    assert!(!harness.transcript().is_blocked());
}

#[test]
fn answers_that_do_not_fit_are_refused_and_the_question_stays() {
    let mut harness = ready();
    prompt(&mut harness, 0, "test");
    let single = ask_question(&mut harness, 9, false);
    for answers in [
        vec![],
        vec!["  ".to_owned()],
        vec!["Running command".to_owned(), "Git branch".to_owned()],
        vec!["x".repeat(sheprd_chat_protocol::MAX_ANSWER_TEXT + 1)],
    ] {
        harness.command(
            0,
            Command::AnswerQuestion {
                answers,
                permission: single,
            },
        );
        assert_eq!(rejection(&harness), Some(Rejection::InvalidAnswer));
    }
    assert!(harness.transcript().is_blocked());
    assert!(harness.sent().iter().all(|message| message["id"] != 9));
}

#[test]
fn a_permission_without_a_question_takes_only_its_options() {
    let mut harness = ready();
    prompt(&mut harness, 0, "test");
    let permission = ask_permission(&mut harness, 5);
    assert_eq!(harness.transcript().permissions[0].question, None);
    harness.command(
        0,
        Command::AnswerQuestion {
            answers: vec!["yes".to_owned()],
            permission,
        },
    );
    assert_eq!(rejection(&harness), Some(Rejection::UnknownOption));
    assert!(harness.transcript().is_blocked());
}
