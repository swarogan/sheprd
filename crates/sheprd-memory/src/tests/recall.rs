use super::*;

fn seeded() -> Memory {
    let mut memory = Memory::in_memory().unwrap();
    for text in [
        "Snapshot tests flake when TMPDIR is long",
        "The relay speaks QUIC with a WebSocket fallback",
        "Run just test-core with an absolute path to herdr-core",
        "The theme picker reads the config on every open",
    ] {
        memory.add(&fact(text), NOW).unwrap();
    }
    memory
}

#[test]
fn a_prompt_recalls_entries_that_share_two_of_its_words() {
    let mut memory = seeded();
    let found = memory
        .recall(
            Scope::All,
            "The snapshot tests are failing again, can you look at the TMPDIR handling?",
            3,
        )
        .unwrap();
    assert_eq!(texts(&found), ["Snapshot tests flake when TMPDIR is long"]);
    // One shared word in a long prompt is not enough.
    let found = memory
        .recall(
            Scope::All,
            "Please add a fallback font for the sidebar badges",
            3,
        )
        .unwrap();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_prompt_of_one_keyword_needs_only_that_one() {
    let mut memory = seeded();
    assert_eq!(
        texts(&memory.recall(Scope::All, "what about quic?", 3).unwrap()),
        ["The relay speaks QUIC with a WebSocket fallback"]
    );
    assert!(
        memory
            .recall(Scope::All, "and the it of", 3)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn recall_leaves_entries_unused() {
    let mut memory = seeded();
    let found = memory.recall(Scope::All, "relay quic", 3).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(memory.get(found[0].id, NOW).unwrap().unwrap().uses, 1);
}

#[test]
fn keywords_drop_short_common_and_repeated_words() {
    assert_eq!(
        crate::recall::keywords("Fix the CI: the CI fails on a Mac, fix it"),
        ["fix", "fails", "mac"]
    );
}
