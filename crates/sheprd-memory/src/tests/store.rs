use super::*;
use std::path::PathBuf;

#[test]
fn repository_entries_reach_every_branch_and_branch_entries_stay_there() {
    let mut memory = Memory::in_memory().unwrap();
    memory.add(&fact("Run just test-core"), NOW).unwrap();
    let on_a = New {
        branch: Some("feat/a"),
        kind: Kind::Handoff,
        ..fact("Halfway through the parser")
    };
    memory.add(&on_a, NOW + 1).unwrap();
    let on_b = New {
        branch: Some("feat/b"),
        ..fact("The fixture is stale")
    };
    memory.add(&on_b, NOW + 2).unwrap();

    let here = memory
        .list(
            Scope::Here {
                branch: Some("feat/a"),
            },
            50,
        )
        .unwrap();
    assert_eq!(
        texts(&here),
        ["Halfway through the parser", "Run just test-core"]
    );
    let detached = memory.list(Scope::Here { branch: None }, 50).unwrap();
    assert_eq!(texts(&detached), ["Run just test-core"]);
    assert_eq!(memory.list(Scope::All, 50).unwrap().len(), 3);
}

#[test]
fn the_same_text_in_the_same_scope_is_kept_once() {
    let mut memory = Memory::in_memory().unwrap();
    let first = memory.add(&fact("  CI runs on macOS  "), NOW).unwrap();
    let again = New {
        kind: Kind::Gotcha,
        ..fact("CI runs on macOS")
    };
    assert_eq!(
        memory.add(&again, NOW + 5).unwrap(),
        Added::Again(first.id())
    );
    let entries = memory.list(Scope::All, 10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].kind, entries[0].used), (Kind::Gotcha, NOW + 5));
    let on_branch = New {
        branch: Some("main"),
        ..fact("CI runs on macOS")
    };
    assert!(matches!(
        memory.add(&on_branch, NOW).unwrap(),
        Added::New(_)
    ));
}

#[test]
fn a_name_updates_its_entry_in_place() {
    let mut memory = Memory::in_memory().unwrap();
    let first = New {
        name: Some("build-commands"),
        kind: Kind::Command,
        details: Some("Long story"),
        pinned: true,
        ..fact("just build")
    };
    let id = memory.add(&first, NOW).unwrap().id();
    let update = New {
        name: Some("build-commands"),
        kind: Kind::Command,
        ..fact("just build-release")
    };
    assert_eq!(memory.add(&update, NOW + 1).unwrap(), Added::Updated(id));
    let entry = memory.get(id, NOW + 2).unwrap().unwrap();
    assert_eq!(entry.text, "just build-release");
    // An update that does not pin, or give details, leaves neither behind.
    assert!(entry.pinned);
    assert_eq!(entry.details, None);
    assert_eq!(entry.updated, NOW + 1);
    // Another entry's text is refused rather than merged into this one.
    let other = memory.add(&fact("other"), NOW).unwrap().id();
    let clash = New {
        name: Some("build-commands"),
        ..fact("other")
    };
    assert!(matches!(memory.add(&clash, NOW), Err(Error::SameText { id }) if id == other));
}

#[test]
fn names_are_plain() {
    let mut memory = Memory::in_memory().unwrap();
    for bad in ["", "Build", "has space", &"x".repeat(MAX_NAME + 1)] {
        let new = New {
            name: Some(bad),
            ..fact("x")
        };
        assert!(
            matches!(memory.add(&new, NOW), Err(Error::Name { .. })),
            "{bad:?}"
        );
    }
    let good = New {
        name: Some("ci/macos-runners_2.x"),
        ..fact("x")
    };
    memory.add(&good, NOW).unwrap();
}

#[test]
fn get_reads_details_and_counts_the_use() {
    let mut memory = Memory::in_memory().unwrap();
    let new = New {
        details: Some("Step one, step two."),
        ..fact("Release steps")
    };
    let id = memory.add(&new, NOW).unwrap().id();
    let entry = memory.get(id, NOW + 3).unwrap().unwrap();
    assert_eq!(entry.details.as_deref(), Some("Step one, step two."));
    assert_eq!((entry.uses, entry.used), (1, NOW + 3));
    assert_eq!(memory.get(id + 100, NOW).unwrap(), None);
}

#[test]
fn a_forgotten_id_is_never_given_to_a_new_entry() {
    let mut memory = Memory::in_memory().unwrap();
    memory.add(&fact("first"), NOW).unwrap();
    let last = memory.add(&fact("second"), NOW).unwrap().id();
    assert!(memory.forget(last).unwrap());
    let next = memory.add(&fact("third"), NOW).unwrap().id();
    assert!(next > last, "{next} reused {last}");
    assert!(!memory.forget(last).unwrap());
    assert_eq!(memory.list(Scope::All, 10).unwrap().len(), 2);
}

#[test]
fn sizes_are_bounded_and_text_required() {
    let mut memory = Memory::in_memory().unwrap();
    assert!(matches!(memory.add(&fact(" \n "), NOW), Err(Error::Empty)));
    let long = "x".repeat(MAX_TEXT + 1);
    assert!(matches!(
        memory.add(&fact(&long), NOW),
        Err(Error::TooLong { what: "text", .. })
    ));
    let details = "y".repeat(MAX_DETAILS + 1);
    let new = New {
        details: Some(&details),
        ..fact("x")
    };
    assert!(matches!(
        memory.add(&new, NOW),
        Err(Error::TooLong {
            what: "details",
            ..
        })
    ));
    memory.add(&fact(&"x".repeat(MAX_TEXT)), NOW).unwrap();
}

#[test]
fn unsafe_text_is_refused_wherever_it_is() {
    let mut memory = Memory::in_memory().unwrap();
    let new = New {
        details: Some("token ghp_abcdefghijklmnopqrstuvwxyz0123"),
        ..fact("fine")
    };
    assert!(matches!(
        memory.add(&new, NOW),
        Err(Error::Unsafe(Unsafe::Secret))
    ));
    assert!(memory.list(Scope::All, 10).unwrap().is_empty());
}

#[test]
fn pins_come_first_survive_eviction_and_are_capped() {
    let mut memory = Memory::in_memory().unwrap();
    let pinned = New {
        pinned: true,
        ..fact("keep me")
    };
    let kept = memory.add(&pinned, NOW).unwrap().id();
    for n in 0..MAX_ENTRIES {
        memory
            .add(&fact(&format!("entry {n}")), NOW + 1 + i64::from(n))
            .unwrap();
    }
    let count: i64 = memory
        .connection()
        .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, i64::from(MAX_ENTRIES));
    let first = &memory.list(Scope::All, 1).unwrap()[0];
    assert_eq!(first.id, kept);
    // The oldest unpinned entry went instead.
    assert!(memory.search(Scope::All, "entry 0", 1, None).unwrap()[0].text != "entry 0");

    let mut memory = Memory::in_memory().unwrap();
    for n in 0..MAX_PINNED {
        let text = format!("pin {n}");
        let new = New {
            pinned: true,
            ..fact(&text)
        };
        memory.add(&new, NOW).unwrap();
    }
    let one_more = memory.add(&fact("one more"), NOW).unwrap().id();
    assert!(matches!(
        memory.pin(one_more, true),
        Err(Error::TooManyPinned { .. })
    ));
    assert!(!memory.get(one_more, NOW).unwrap().unwrap().pinned);
    assert!(memory.pin(1, false).unwrap());
    assert!(memory.pin(one_more, true).unwrap());
}

#[test]
fn every_worktree_of_a_repository_opens_one_file() {
    let state = tempfile::tempdir().unwrap();
    let git_dir = PathBuf::from("/code/app/.git");
    let path = path_in(state.path(), &git_dir);
    assert!(path.ends_with("memory.sqlite3"));
    Memory::open(&path)
        .unwrap()
        .add(&fact("Shared"), NOW)
        .unwrap();
    let mut other = Memory::open(&path).unwrap();
    assert_eq!(
        other.search(Scope::All, "shared", 5, None).unwrap().len(),
        1
    );
}

#[test]
fn a_newer_schema_is_refused() {
    let state = tempfile::tempdir().unwrap();
    let path = path_in(state.path(), std::path::Path::new("/r/.git"));
    drop(Memory::open(&path).unwrap());
    rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    assert!(matches!(
        Memory::open(&path),
        Err(Error::NewerSchema { found, known: SCHEMA_VERSION }) if found == SCHEMA_VERSION + 1
    ));
}

#[test]
fn kinds_parse_from_their_names() {
    for kind in KINDS {
        assert_eq!(kind.as_str().parse::<Kind>().unwrap(), *kind);
    }
    assert!(matches!("note".parse::<Kind>(), Err(Error::Kind(name)) if name == "note"));
}
