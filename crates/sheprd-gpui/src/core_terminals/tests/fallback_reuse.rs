//! A shell the core refused as already running is held by the address the
//! core lists it under, not by the name the window asked for: everything that
//! acts on the shell's pane matches its id against the core's addresses.

use crate::core_terminals::fallback::reused_id;
use sheprd_backend::{AgentStatus, RunState, TerminalId, TerminalInfo};

/// The name the stand-in shell is spawned with, as `fallback::SHELL` has it.
const SHELL: &str = "herdr-ui";

fn listed(address: &str, name: Option<&str>) -> TerminalInfo {
    TerminalInfo {
        agent: AgentStatus::Idle,
        cwd: None,
        foreground: None,
        id: TerminalId::new(address).unwrap(),
        labels: Default::default(),
        name: name.map(str::to_owned),
        reporter: Some("pi".to_owned()),
        state: RunState::Running,
        title: None,
    }
}

#[test]
fn a_reused_shell_is_held_by_its_address() {
    // The core lists only live terminals, and names are unique among them,
    // so the shell the refusal named is the one this finds.
    let id = reused_id(&[listed("t6", Some(SHELL))]).unwrap();
    assert_eq!(
        id.as_str(),
        "t6",
        "a note, a notification, and an editor ask all match addresses"
    );
}

#[test]
fn a_reused_shell_is_not_confused_with_another_terminal() {
    let id = reused_id(&[
        listed("t1", Some("build")),
        listed("t6", Some(SHELL)),
        listed("t9", None),
    ])
    .unwrap();
    assert_eq!(id.as_str(), "t6");
}

#[test]
fn a_shell_that_ended_meanwhile_keeps_its_name() {
    // The core resolves a name too, so the window still attaches rather than
    // giving up on a shell that went between the refusal and the list.
    for gone in [vec![], vec![listed("t1", Some("build"))]] {
        assert_eq!(reused_id(&gone).unwrap().as_str(), SHELL);
    }
}
