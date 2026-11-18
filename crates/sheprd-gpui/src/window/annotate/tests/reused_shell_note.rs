//! A note on the stand-in shell shown in place of Herdr 1 is aimed at the
//! address the core gave that shell. A restart that found an earlier launch's
//! shell live holds it by the name it was spawned with, and delivery matches a
//! note's terminal against each record's address only, so a note aimed at the
//! name is copied instead of typed in while the agent runs.
//!
//! The window's own path is not reachable here: `CoreTerminals::reported`
//! answers only once a core is connected, and the in-crate hub seam
//! (`Hub::with_join`) can only hand back a failed join. What is reachable, and
//! what every reused shell's notes turn on, is the target this choice makes.

use crate::core_terminals::Host;
use crate::window::annotate::core::shell_note_agent;
use sheprd_backend::{AgentStatus, RunState, TerminalId, TerminalInfo};

/// The name the stand-in shell is spawned with, as `fallback::SHELL` has it.
const SHELL: &str = "herdr-ui";

/// What the core lists of a live stand-in shell running an agent that reports
/// its status: addressed as the core assigned it, named as the window asked.
fn listed(address: &str) -> TerminalInfo {
    TerminalInfo {
        agent: AgentStatus::Idle,
        cwd: None,
        foreground: None,
        id: TerminalId::new(address).unwrap(),
        labels: Default::default(),
        name: Some(SHELL.to_owned()),
        reporter: Some("pi".to_owned()),
        state: RunState::Running,
        title: None,
    }
}

#[test]
fn a_note_on_the_stand_in_shell_is_aimed_at_its_address() {
    let agent = shell_note_agent(&listed("t7")).unwrap();
    assert_eq!(agent.terminal.as_str(), "t7");
    // The id the window holds for a reused shell is this name, and no record
    // answers to it: `reply::when_free` searches `record.address` only.
    assert_ne!(agent.terminal.as_str(), SHELL);
    assert_eq!(agent.agent, "pi");
    assert_eq!(agent.host, Host::Local);
}

#[test]
fn a_stand_in_shell_that_reports_no_agent_takes_no_note() {
    let mut shell = listed("t7");
    shell.reporter = None;
    assert!(shell_note_agent(&shell).is_none());
}
