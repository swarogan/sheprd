#![allow(clippy::unwrap_used)]

use super::*;
use sheprd_chat_protocol::{
    Choice, CommandId, ConfigOption, Entry, EntryId, Mode, Modes, Queued, Turn,
};

fn claude(current: &str) -> Transcript {
    let mode = |id: &str| Mode {
        description: None,
        id: id.to_owned(),
        name: id.to_owned(),
    };
    Transcript {
        agent: "Claude".to_owned(),
        modes: Some(Modes {
            available: vec![mode("default"), mode("plan"), mode("bypassPermissions")],
            current: current.to_owned(),
        }),
        ..Transcript::default()
    }
}

fn with_mode_option(current: &str) -> Transcript {
    let choice = |id: &str| Choice {
        description: None,
        group: None,
        id: id.to_owned(),
        name: id.to_owned(),
    };
    Transcript {
        agent: "Codex".to_owned(),
        options: vec![ConfigOption {
            category: OptionCategory::Mode,
            description: None,
            id: "mode".to_owned(),
            kind: OptionKind::Select {
                choices: vec![choice("ask"), choice("full-access")],
                current: current.to_owned(),
            },
            name: "Mode".to_owned(),
        }],
        ..Transcript::default()
    }
}

#[test]
fn a_chosen_mode_is_read_from_either_kind_of_mode() {
    let set_mode = Command::SetMode {
        id: "bypassPermissions".to_owned(),
    };
    assert_eq!(
        chosen(&claude("default"), &set_mode),
        Some("bypassPermissions")
    );
    let option = Command::SetOption {
        id: "mode".to_owned(),
        value: OptionValue::Choice {
            id: "full-access".to_owned(),
        },
    };
    assert_eq!(
        chosen(&with_mode_option("ask"), &option),
        Some("full-access")
    );
    // Another setting, such as the model, is not a mode.
    let model = Command::SetOption {
        id: "model".to_owned(),
        value: OptionValue::Choice {
            id: "opus".to_owned(),
        },
    };
    assert_eq!(chosen(&with_mode_option("ask"), &model), None);
    assert_eq!(chosen(&claude("default"), &Command::Cancel), None);
}

#[test]
fn a_fresh_chat_switches_only_to_a_mode_its_agent_offers() {
    assert_eq!(
        switch_to(&claude("default"), "bypassPermissions"),
        Some(Command::SetMode {
            id: "bypassPermissions".to_owned()
        })
    );
    assert_eq!(
        switch_to(&claude("bypassPermissions"), "bypassPermissions"),
        None
    );
    assert_eq!(switch_to(&claude("default"), "full-access"), None);
    assert_eq!(
        switch_to(&with_mode_option("ask"), "full-access"),
        Some(Command::SetOption {
            id: "mode".to_owned(),
            value: OptionValue::Choice {
                id: "full-access".to_owned()
            },
        })
    );
    assert_eq!(switch_to(&Transcript::default(), "plan"), None);
}

#[test]
fn a_chat_under_way_or_only_watched_is_not_fresh() {
    assert!(fresh(&claude("default")));
    let mut prompted = claude("default");
    prompted.entries.push(Entry {
        id: EntryId(1),
        kind: EntryKind::User {
            text: "hi".to_owned(),
            files: 0,
        },
    });
    assert!(!fresh(&prompted));
    let mut running = claude("default");
    running.turn = Some(Turn { started_ms: 1 });
    assert!(!fresh(&running));
    let mut queued = claude("default");
    queued.queued.push(Queued {
        attachments: Vec::new(),
        id: CommandId(uuid::Uuid::nil()),
        text: "hi".to_owned(),
    });
    assert!(!fresh(&queued));
    // Trimmed past its last prompt, an old conversation keeps only replies.
    let mut trimmed = claude("default");
    trimmed.truncated = true;
    trimmed.entries.push(Entry {
        id: EntryId(9),
        kind: EntryKind::Agent {
            text: "done".to_owned(),
        },
    });
    assert!(!fresh(&trimmed));
    let mut remembered = Remembered::default();
    remembered.set("Claude", "bypassPermissions");
    let mut settled = None;
    assert_eq!(settle(&trimmed, &mut settled, Some(&remembered)), None);
    assert_eq!(settled.as_deref(), Some("Claude"));
    let mut watched = claude("default");
    watched.read_only = true;
    assert!(!fresh(&watched));
}

#[test]
fn modes_are_remembered_per_agent_and_bounded() {
    let mut remembered = Remembered::default();
    remembered.set("Claude", "bypassPermissions");
    remembered.set("Codex", "full-access");
    remembered.set("Claude", "");
    remembered.set("Claude", &"x".repeat(MAX_FIELD_BYTES + 1));
    assert_eq!(remembered.mode("Claude"), Some("bypassPermissions"));
    assert_eq!(remembered.mode("Codex"), Some("full-access"));
    for index in 0..MAX_AGENTS * 2 {
        remembered.set(&format!("agent-{index}"), "mode");
    }
    assert_eq!(remembered.modes.len(), MAX_AGENTS);
    // A known agent still changes its mode at the limit.
    remembered.set("Claude", "default");
    assert_eq!(remembered.mode("Claude"), Some("default"));
}

#[test]
fn the_choices_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    let mut remembered = Remembered::at(Some(path.clone()));
    remembered.set("Claude", "bypassPermissions");
    remembered.finish();
    assert_eq!(
        Remembered::at(Some(path)).mode("Claude"),
        Some("bypassPermissions")
    );
}

#[test]
fn damaged_files_remember_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    std::fs::write(&path, b"{not json").unwrap();
    assert!(Remembered::at(Some(path.clone())).modes.is_empty());
    std::fs::write(&path, br#"{"modes":{"Claude":""}}"#).unwrap();
    assert!(Remembered::at(Some(path)).modes.is_empty());
}

#[test]
fn each_agent_of_a_chat_is_settled_once() {
    let mut remembered = Remembered::default();
    remembered.set("Claude", "bypassPermissions");
    remembered.set("Codex", "full-access");
    let mut settled = None;
    // Nothing to settle until the agent lists its modes.
    assert_eq!(
        settle(&Transcript::default(), &mut settled, Some(&remembered)),
        None
    );
    assert_eq!(settled, None);
    assert_eq!(
        settle(&claude("default"), &mut settled, Some(&remembered)),
        Some(Command::SetMode {
            id: "bypassPermissions".to_owned()
        })
    );
    // Choosing another mode in this chat afterwards is left alone.
    assert_eq!(
        settle(&claude("plan"), &mut settled, Some(&remembered)),
        None
    );
    // Switching agents before the first prompt settles the new agent.
    assert!(settle(&with_mode_option("ask"), &mut settled, Some(&remembered)).is_some());
}

#[test]
fn a_chat_already_under_way_is_settled_without_switching() {
    let mut remembered = Remembered::default();
    remembered.set("Claude", "bypassPermissions");
    let mut restored = claude("default");
    restored.entries.push(Entry {
        id: EntryId(1),
        kind: EntryKind::User {
            text: "hi".to_owned(),
            files: 0,
        },
    });
    let mut settled = None;
    assert_eq!(settle(&restored, &mut settled, Some(&remembered)), None);
    assert_eq!(settled.as_deref(), Some("Claude"));
    assert_eq!(settle(&claude("default"), &mut None, None), None);
}
