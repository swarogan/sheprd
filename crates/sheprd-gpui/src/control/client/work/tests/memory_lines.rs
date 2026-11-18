use super::*;
use sheprd_memory::{Entry, Kind};

fn entry(id: i64, branch: Option<&str>, kind: Kind, text: &str) -> Entry {
    Entry {
        id,
        branch: branch.map(str::to_owned),
        name: None,
        kind,
        text: text.to_owned(),
        details: None,
        pinned: false,
        uses: 0,
        created: 0,
        used: 0,
        updated: 0,
    }
}

#[test]
fn memory_entries_read_one_per_line() {
    assert_eq!(
        memory::describe(&[
            entry(3, Some("feat/x"), Kind::Handoff, "Parser half done"),
            entry(1, None, Kind::Command, "Run\n  just test-core\n"),
        ]),
        "#3 [handoff] (branch \"feat/x\") \"Parser half done\"\n#1 [command] \"Run just test-core\""
    );
}

#[test]
fn one_memory_entry_reads_in_full() {
    let mut full = entry(5, None, Kind::Decision, "Use FTS5");
    full.name = Some("search".into());
    full.pinned = true;
    full.details = Some("Vectors can come later.".into());
    assert_eq!(
        memory::describe_one(&full),
        "#5 [decision, pinned] search: \"Use FTS5\" [+details]\n\nUse FTS5\n\nVectors can come later."
    );
}
