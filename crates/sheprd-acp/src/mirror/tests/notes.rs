//! A chat's log, mirrored, shows what the person wrote, not Sheprd's notes.

use super::*;
use crate::context::{Context, Facts, Prompts};

fn said(text: &str) -> Entry {
    Entry {
        id: EntryId(1),
        kind: EntryKind::User {
            text: text.to_owned(),
            files: 0,
        },
    }
}

#[test]
fn notes_sent_to_the_agent_stay_hidden_in_a_mirror() {
    let with_note = |words: &str| {
        let mut transcript = Transcript::new("Claude".to_owned());
        let note = Context::new(
            Prompts::with_instructions("Be brief."),
            Facts::default(),
            Default::default(),
        )
        .before(None)
        .unwrap();
        transcript.entries = vec![said(&format!("{note}\n{words}"))];
        transcript
    };
    let mut mirror = Mirror::new(with_note("first"), None);
    assert_eq!(mirror.transcript.entries, [said("first")]);
    mirror.update(with_note("edited"));
    assert_eq!(mirror.transcript.entries, [said("edited")]);
}
