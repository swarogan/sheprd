//! A chat its agent has not titled is named on a phone by its first prompt.

use super::*;
use sheprd_chat_protocol::{Entry, EntryId, EntryKind, Transcript};

fn user(id: u64, text: &str) -> Entry {
    Entry {
        id: EntryId(id),
        kind: EntryKind::User {
            text: text.into(),
            files: 0,
        },
    }
}

#[test]
fn the_first_prompt_that_says_something_names_the_chat() {
    let mut transcript = Transcript::new("Claude");
    assert_eq!(chat::first_prompt(&transcript), None);
    transcript.entries = vec![
        Entry {
            id: EntryId(1),
            kind: EntryKind::Agent {
                text: "Hello".into(),
            },
        },
        user(2, "  \n "),
        user(3, "Fix the\nlogin  flow"),
        user(4, "Later prompt"),
    ];
    assert_eq!(
        chat::first_prompt(&transcript).as_deref(),
        Some("Fix the login flow")
    );
}

#[test]
fn a_long_first_prompt_is_cut_to_one_short_line() {
    let mut transcript = Transcript::new("Claude");
    transcript.entries = vec![user(1, &"word ".repeat(100))];
    let prompt = chat::first_prompt(&transcript).unwrap();
    assert_eq!(prompt.chars().count(), 80);
    assert!(prompt.ends_with('…'));
}
