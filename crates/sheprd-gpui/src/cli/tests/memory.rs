use super::*;
use control::{ControlCommand, MemoryAdd, MemoryCommand};
use sheprd_memory::Kind;

fn memory(args: &[&str]) -> Result<ControlCommand, CliError> {
    let mut all = vec!["memory"];
    all.extend_from_slice(args);
    match LaunchOptions::parse(all)?.mode {
        LaunchMode::Control(invocation) => Ok(invocation.command),
        mode => panic!("{mode:?}"),
    }
}

fn list(all: bool, limit: usize) -> ControlCommand {
    ControlCommand::Memory(MemoryCommand::List { all, limit })
}

#[test]
fn memory_lists_by_default_and_takes_scope_and_limit() {
    assert_eq!(memory(&[]).unwrap(), list(false, 20));
    assert_eq!(
        memory(&["list", "--all", "--limit", "5"]).unwrap(),
        list(true, 5)
    );
    // The list flags work without the verb too.
    assert_eq!(memory(&["--all"]).unwrap(), list(true, 20));
    assert_eq!(memory(&["--limit", "5"]).unwrap(), list(false, 5));
    assert!(memory(&["list", "--limit", "0"]).is_err());
    assert!(memory(&["list", "--limit", "201"]).is_err());
}

#[test]
fn memory_search_joins_its_words() {
    assert_eq!(
        memory(&["search", "test", "--all", "core"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Search {
            query: "test core".into(),
            all: true,
            limit: 20,
        })
    );
    assert!(memory(&["search"]).is_err());
}

#[test]
fn memory_add_takes_its_options() {
    assert_eq!(
        memory(&[
            "add",
            "--kind",
            "gotcha",
            "--branch",
            "--pin",
            "--name",
            "ci-speed",
            "--details",
            "The long story",
            "CI",
            "is",
            "slow",
        ])
        .unwrap(),
        ControlCommand::Memory(MemoryCommand::Add(MemoryAdd {
            branch: true,
            name: Some("ci-speed".into()),
            kind: Kind::Gotcha,
            text: "CI is slow".into(),
            details: Some("The long story".into()),
            pinned: true,
        }))
    );
    assert_eq!(
        memory(&["add", "plain", "fact"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Add(MemoryAdd {
            branch: false,
            name: None,
            kind: Kind::Fact,
            text: "plain fact".into(),
            details: None,
            pinned: false,
        }))
    );
    assert!(memory(&["add", "--kind", "note", "x"]).is_err());
    assert!(memory(&["add"]).is_err());
    let long = "x".repeat(501);
    assert!(memory(&["add", &long]).is_err());
    assert!(memory(&["add", "--limit", "3", "text"]).is_err());
}

#[test]
fn memory_verbs_on_one_entry_take_an_id() {
    assert_eq!(
        memory(&["get", "#12"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Get { id: 12 })
    );
    assert_eq!(
        memory(&["pin", "3"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Pin {
            id: 3,
            pinned: true
        })
    );
    assert_eq!(
        memory(&["unpin", "3"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Pin {
            id: 3,
            pinned: false
        })
    );
    assert_eq!(
        memory(&["forget", "12"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Forget { id: 12 })
    );
    assert!(memory(&["forget", "x"]).is_err());
    assert!(memory(&["get"]).is_err());
    assert!(memory(&["forget", "12", "--all"]).is_err());
    assert!(memory(&["remember", "x"]).is_err());
}

#[test]
fn memory_index_takes_a_budget() {
    assert_eq!(
        memory(&["index"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Index { chars: 3000 })
    );
    assert_eq!(
        memory(&["index", "--chars", "800"]).unwrap(),
        ControlCommand::Memory(MemoryCommand::Index { chars: 800 })
    );
    assert!(memory(&["index", "--chars", "10"]).is_err());
}
