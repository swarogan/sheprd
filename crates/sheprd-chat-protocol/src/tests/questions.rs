use serde_json::json;

use super::*;

#[test]
fn a_permission_from_an_older_program_has_no_question() {
    let permission: Permission = serde_json::from_value(json!({
        "id": 3, "options": [], "title": "Run it",
    }))
    .unwrap();
    assert_eq!(permission.question, None);
    let written = serde_json::to_value(&permission).unwrap();
    assert!(written.get("question").is_none());
}

#[test]
fn answering_a_question_is_written_as_serde_names_it() {
    let command = Command::AnswerQuestion {
        answers: vec!["Git branch".to_owned()],
        permission: PermissionId(4),
    };
    assert_eq!(
        serde_json::to_value(&command).unwrap(),
        json!({"command": "answer_question", "answers": ["Git branch"], "permission": 4})
    );
}

#[test]
fn answers_must_fit_the_question() {
    let mut question = Question {
        options: vec![QuestionOption {
            description: None,
            label: "A".to_owned(),
        }],
        text: "Pick".to_owned(),
        ..Question::default()
    };
    let answers = |list: &[&str]| {
        list.iter()
            .map(|answer| (*answer).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(question.check(&answers(&["A"])), Ok(()));
    assert_eq!(question.check(&answers(&["my own words"])), Ok(()));
    assert_eq!(question.check(&answers(&[])), Err(Rejection::InvalidAnswer));
    assert_eq!(
        question.check(&answers(&[" "])),
        Err(Rejection::InvalidAnswer)
    );
    assert_eq!(
        question.check(&answers(&["A", "B"])),
        Err(Rejection::InvalidAnswer)
    );
    question.multiple = true;
    assert_eq!(question.check(&answers(&["A", "B"])), Ok(()));
    let many = vec!["A".to_owned(); MAX_QUESTION_CHOICES + 1];
    assert_eq!(question.check(&many), Err(Rejection::InvalidAnswer));
    let long = vec!["x".repeat(MAX_ANSWER_TEXT + 1)];
    assert_eq!(question.check(&long), Err(Rejection::InvalidAnswer));
}
