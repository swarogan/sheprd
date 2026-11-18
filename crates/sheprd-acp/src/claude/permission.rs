//! Claude Code's `can_use_tool` questions as ACP permission requests, and
//! the answers back.

use acp::v1 as wire;
use serde_json::{Map, Value, json};

use sheprd_chat_protocol::{Question, QuestionOption};

use super::tools;
use crate::harness::out;

/// What a chosen option tells Claude.
#[derive(Clone, Debug, PartialEq)]
enum Choice {
    /// Run the tool, with `input` in place of what Claude asked for when
    /// set, and the permission `updates` Claude suggested when set.
    Allow {
        input: Option<Value>,
        updates: Option<Value>,
    },
    Deny {
        message: &'static str,
    },
    /// Pick `label` for the `AskUserQuestion` question being asked, then
    /// ask the next one or answer them all.
    Answer {
        label: String,
    },
}

/// A question waiting for its answer.
#[derive(Debug)]
pub(super) struct Ask {
    choices: Vec<(String, Choice)>,
    /// The control request it answers.
    control: String,
    input: Value,
    /// Where an `AskUserQuestion` call is: the question on screen and the
    /// answers so far, keyed by question text as Claude Code expects.
    questions: Option<(usize, Map<String, Value>)>,
    /// The question on screen as clients show it, which they may answer in
    /// their own words or with several choices.
    shown: Option<Box<Question>>,
    /// What the permission card says.
    title: String,
}

/// What the session's answer leads to.
#[derive(Debug)]
pub(super) enum Answered {
    /// The control response for Claude.
    Reply(Value),
    /// The next question of the same call, to ask with these options.
    Next(Vec<wire::PermissionOption>, Ask),
}

/// The options to offer for one `can_use_tool` request, and the question
/// that keeps them.
pub(super) fn ask(control: String, request: &Value) -> (Vec<wire::PermissionOption>, Ask) {
    let tool = request
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let input = request.get("input").cloned().unwrap_or(Value::Null);
    if tool == tools::QUESTION
        && let Some(asked) = question(control.clone(), input.clone(), 0, Map::new())
    {
        return asked;
    }
    let suggestions = request
        .get("permission_suggestions")
        .and_then(Value::as_array)
        .filter(|suggestions| !suggestions.is_empty());
    let options = match tool {
        tools::EXIT_PLAN => exit_plan(),
        tools::QUESTION => vec![reject(
            "reject",
            "Answer in the chat",
            "Ask these questions in your reply instead; the user will answer there.",
        )],
        _ => {
            let mut options = vec![allow_once("allow", "Yes")];
            if let Some(suggestions) = suggestions {
                options.push((
                    wire::PermissionOption::new(
                        "always",
                        always_name(suggestions),
                        wire::PermissionOptionKind::AllowAlways,
                    ),
                    Choice::Allow {
                        input: None,
                        updates: Some(Value::Array(suggestions.clone())),
                    },
                ));
            }
            options.push(reject("reject", "No", "The user refused this."));
            options
        }
    };
    let title = tools::title(tool, &input);
    offer(options, control, input, None, title)
}

fn offer(
    options: Vec<Offered>,
    control: String,
    input: Value,
    questions: Option<(usize, Map<String, Value>)>,
    title: String,
) -> (Vec<wire::PermissionOption>, Ask) {
    let shown = questions
        .as_ref()
        .and_then(|(index, _)| shown_question(&input, *index))
        .map(Box::new);
    let (offered, choices) = options
        .into_iter()
        .map(|(option, choice)| {
            let id = option.option_id.0.to_string();
            (option, (id, choice))
        })
        .unzip();
    (
        offered,
        Ask {
            choices,
            control,
            input,
            questions,
            shown,
            title,
        },
    )
}

impl Ask {
    /// What the permission card says: the tool call's title, or the
    /// question being asked.
    pub(super) fn title(&self) -> &str {
        &self.title
    }

    /// The permission request's `_meta`: the question on screen in full,
    /// when it is one a client may answer itself.
    pub(super) fn meta(&self) -> Option<wire::Meta> {
        let question = serde_json::to_value(self.shown.as_ref()?).ok()?;
        let mut meta = wire::Meta::new();
        meta.insert(out::QUESTION.to_owned(), question);
        Some(meta)
    }

    /// The answer a client gave to the question on screen in its own words,
    /// several choices joined as Claude Code joins them.
    fn answered(&self, answers: &[&str]) -> Option<Choice> {
        self.shown.as_ref()?;
        (!answers.is_empty()).then(|| Choice::Answer {
            label: answers.join(", "),
        })
    }

    /// The control response for the session's answer: the chosen option,
    /// the next question when one is left, or a refusal that also stops
    /// the turn when the question was dropped.
    pub(super) fn answer(self, result: &Result<Value, wire::Error>) -> Answered {
        let chosen = result
            .as_ref()
            .ok()
            .and_then(|value| {
                serde_json::from_value::<wire::RequestPermissionResponse>(value.clone()).ok()
            })
            .and_then(|response| match response.outcome {
                wire::RequestPermissionOutcome::Selected(selected) => Some(selected),
                _ => None,
            })
            .and_then(|selected| {
                let id = selected.option_id.0.as_ref();
                if id == out::ANSWERS {
                    let answers = selected.meta.as_ref()?.get(out::ANSWERS)?.as_array()?;
                    let answers: Vec<&str> = answers.iter().filter_map(Value::as_str).collect();
                    return self.answered(&answers);
                }
                self.choices
                    .iter()
                    .find(|(known, _)| known == id)
                    .map(|(_, choice)| choice.clone())
            });
        let decision = match chosen {
            Some(Choice::Allow { input, updates }) => {
                let mut decision = json!({
                    "behavior": "allow",
                    "updatedInput": input.unwrap_or(self.input),
                });
                if let Some(updates) = updates {
                    decision["updatedPermissions"] = updates;
                }
                decision
            }
            Some(Choice::Deny { message }) => json!({"behavior": "deny", "message": message}),
            Some(Choice::Answer { label }) => {
                let (index, mut answers) = self.questions.unwrap_or_default();
                if let Some(asked) = tools::question_at(&self.input, index) {
                    answers.insert(asked.to_owned(), Value::String(label));
                }
                if let Some(next) = question(
                    self.control.clone(),
                    self.input.clone(),
                    index + 1,
                    answers.clone(),
                ) {
                    let (options, ask) = next;
                    return Answered::Next(options, ask);
                }
                let mut input = self.input.as_object().cloned().unwrap_or_default();
                input.insert("answers".to_owned(), Value::Object(answers));
                json!({"behavior": "allow", "updatedInput": input})
            }
            None => json!({
                "behavior": "deny",
                "message": "The user cancelled.",
                "interrupt": true,
            }),
        };
        Answered::Reply(super::control_success(&self.control, decision))
    }
}

type Offered = (wire::PermissionOption, Choice);

fn allow_once(id: &str, name: &str) -> Offered {
    (
        wire::PermissionOption::new(id.to_owned(), name, wire::PermissionOptionKind::AllowOnce),
        Choice::Allow {
            input: None,
            updates: None,
        },
    )
}

fn reject(id: &str, name: &str, message: &'static str) -> Offered {
    (
        wire::PermissionOption::new(id.to_owned(), name, wire::PermissionOptionKind::RejectOnce),
        Choice::Deny { message },
    )
}

/// Leaving plan mode also picks the mode to code in.
fn exit_plan() -> Vec<Offered> {
    let switch =
        |mode: &str| Some(json!([{"type": "setMode", "mode": mode, "destination": "session"}]));
    vec![
        (
            wire::PermissionOption::new(
                "plan-accept-edits",
                "Yes, and accept edits",
                wire::PermissionOptionKind::AllowAlways,
            ),
            Choice::Allow {
                input: None,
                updates: switch("acceptEdits"),
            },
        ),
        (
            wire::PermissionOption::new(
                "plan-default",
                "Yes, and ask before edits",
                wire::PermissionOptionKind::AllowOnce,
            ),
            Choice::Allow {
                input: None,
                updates: switch("default"),
            },
        ),
        reject(
            "reject",
            "No, keep planning",
            "The user wants to keep planning.",
        ),
    ]
}

/// The question at `index` of an `AskUserQuestion` call, its choices as
/// the options. Claude Code takes every answer at once, so a call with
/// several questions asks them one after another and answers when the
/// last is chosen. A multiple-choice question takes one pick. None once
/// every question is answered, or when the call has no questions.
fn question(
    control: String,
    input: Value,
    index: usize,
    answers: Map<String, Value>,
) -> Option<(Vec<wire::PermissionOption>, Ask)> {
    let asked = tools::question_at(&input, index)?.to_owned();
    let count = tools::question_count(&input);
    let mut options: Vec<Offered> = input
        .get("questions")
        .and_then(|questions| questions.get(index))
        .and_then(|question| question.get("options"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| option.get("label").and_then(Value::as_str))
        .enumerate()
        .map(|(number, label)| {
            (
                out::manual(wire::PermissionOption::new(
                    format!("answer-{number}"),
                    label,
                    wire::PermissionOptionKind::AllowOnce,
                )),
                Choice::Answer {
                    label: label.to_owned(),
                },
            )
        })
        .collect();
    let refusal = if options.is_empty() {
        "Ask this question in your reply instead; the user will answer there."
    } else {
        "The user chose not to answer."
    };
    options.push(reject("reject", "Answer in the chat", refusal));
    let title = if count > 1 {
        format!("{asked} ({}/{count})", index + 1)
    } else {
        asked
    };
    Some(offer(
        options,
        control,
        input,
        Some((index, answers)),
        title,
    ))
}

/// Question `index` of an `AskUserQuestion` call as clients show it.
fn shown_question(input: &Value, index: usize) -> Option<Question> {
    let text = tools::question_at(input, index)?;
    let asked = input.get("questions")?.get(index)?;
    let field =
        |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    Some(Question {
        header: field(asked, "header"),
        multiple: asked.get("multiSelect") == Some(&Value::Bool(true)),
        options: asked
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|option| {
                Some(QuestionOption {
                    description: field(option, "description"),
                    label: field(option, "label")?,
                })
            })
            .collect(),
        text: text.to_owned(),
    })
}

/// What "always" means for the updates Claude suggests.
fn always_name(suggestions: &[Value]) -> String {
    let first = &suggestions[0];
    match first.get("type").and_then(Value::as_str) {
        Some("setMode") => match first.get("mode").and_then(Value::as_str) {
            Some("acceptEdits") => "Yes, and accept edits this session".to_owned(),
            Some("bypassPermissions") => "Yes, and stop asking this session".to_owned(),
            _ => "Yes, and switch mode".to_owned(),
        },
        Some("addRules") => {
            let rules: Vec<String> = first
                .get("rules")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|rule| {
                    let tool = rule.get("toolName")?.as_str()?;
                    Some(match rule.get("ruleContent").and_then(Value::as_str) {
                        Some(content) => format!("{tool}({content})"),
                        None => tool.to_owned(),
                    })
                })
                .collect();
            if rules.is_empty() {
                "Yes, and don't ask again".to_owned()
            } else {
                format!("Yes, and don't ask again for {}", rules.join(", "))
            }
        }
        _ => "Yes, and don't ask again".to_owned(),
    }
}
