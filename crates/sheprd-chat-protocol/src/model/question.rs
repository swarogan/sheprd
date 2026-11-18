//! A question the agent asks with choices, carried beside the permission
//! request that waits for it, so a client can show it whole and answer it
//! with several choices or its own words.

use serde::{Deserialize, Serialize};

use crate::Rejection;

/// Answers one [`Command::AnswerQuestion`](crate::Command::AnswerQuestion)
/// may carry, and the UTF-8 bytes of each.
pub const MAX_QUESTION_CHOICES: usize = 32;
pub const MAX_ANSWER_TEXT: usize = 4 * 1024;

/// The question, its options with what the agent said about each, and
/// whether several may be chosen. A client that does not know it still
/// sees the request's plain options.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// A short label for the question, such as "Tab detail".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(default)]
    pub multiple: bool,
    pub options: Vec<QuestionOption>,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QuestionOption {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub label: String,
}

impl Question {
    /// Checks the answers a client sent: at least one, one only unless
    /// several may be chosen, each non-blank and bounded. An answer that
    /// is not an option's label is the person's own words.
    pub fn check(&self, answers: &[String]) -> Result<(), Rejection> {
        let most = if self.multiple {
            MAX_QUESTION_CHOICES
        } else {
            1
        };
        let fits = (1..=most).contains(&answers.len())
            && answers
                .iter()
                .all(|answer| !answer.trim().is_empty() && answer.len() <= MAX_ANSWER_TEXT);
        if fits {
            Ok(())
        } else {
            Err(Rejection::InvalidAnswer)
        }
    }
}
