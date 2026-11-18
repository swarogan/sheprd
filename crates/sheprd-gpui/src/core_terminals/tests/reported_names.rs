//! The stand-in shell a restart reused is held by the name it was spawned
//! with, so what the core lists of it has to answer to that name and not only
//! to the address the core gave it — otherwise the shell's agent is invisible
//! however loudly it reports.

use crate::{
    browser::Scope,
    core_terminals::{CoreTerminals, Host},
};
use sheprd_backend::{RunState, TerminalId, TerminalInfo};
use sheprd_client::protocol::AgentStatus;

/// The name the stand-in shell is spawned with, as `fallback::SHELL` has it.
const SHELL: &str = "herdr-ui";

/// What the core lists of a live stand-in shell running an agent that reports
/// its own status, as `herdr-core get` reports it.
fn shell(address: &str) -> TerminalInfo {
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
fn a_shell_reused_by_name_is_found_by_that_name() {
    let mut terminals = CoreTerminals::default();
    terminals.infos.list_for_test(vec![shell("t6")]);
    // A restart found `herdr-ui` live and reused it, so the window holds the
    // name it asked for rather than the address the core assigned.
    let by_name = TerminalId::new(SHELL).unwrap();
    assert_eq!(
        terminals
            .listed(&Host::Local, &by_name)
            .map(|info| info.agent),
        Some(AgentStatus::Idle),
        "the reused shell's agent is found under its name"
    );
    // Every other pane is held by its address, which keeps resolving.
    let by_address = TerminalId::new("t6").unwrap();
    assert_eq!(
        terminals
            .listed(&Host::Local, &by_address)
            .map(|info| info.reporter.as_deref()),
        Some(Some("pi"))
    );
}

#[test]
fn a_shell_name_never_reaches_another_host() {
    let mut terminals = CoreTerminals::default();
    terminals.infos.list_for_test(vec![shell("t6")]);
    let remote = Host::Remote(Scope::endpoint("ssh:box"));
    assert!(
        terminals
            .listed(&remote, &TerminalId::new(SHELL).unwrap())
            .is_none(),
        "names repeat across cores, as addresses do"
    );
}

#[test]
fn a_shell_the_core_stopped_listing_is_not_reported() {
    let mut terminals = CoreTerminals::default();
    terminals.infos.list_for_test(vec![shell("t6")]);
    // The core lists only live terminals, so a shell that ended leaves the
    // list and its name resolves to nothing rather than to a stale record.
    terminals.infos.list_for_test(Vec::new());
    assert!(
        terminals
            .listed(&Host::Local, &TerminalId::new(SHELL).unwrap())
            .is_none()
    );
}
