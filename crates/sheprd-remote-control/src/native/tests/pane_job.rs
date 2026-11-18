//! A terminal pane says what runs in front of its shell, so a phone's row
//! shows the command rather than the folder.

use super::*;
use crate::hub::Job;
use model::{Home, Sources};

fn home(layout: &Layout) -> Home {
    Home::project(Sources {
        checkouts: &Default::default(),
        recognized: &Default::default(),
        computer: "Studio",
        fingerprint: "FP",
        layout,
        load: None,
        summaries: HashMap::new(),
        pull_requests: &Default::default(),
        shortcuts_ms: 0,
        tags: Default::default(),
        version: "20261109.1",
        waiting: 0,
    })
}

fn wire_pane(job: Option<Job>) -> serde_json::Value {
    let mut layout = layout(EndpointId::random());
    layout.workspaces[0].terminals[0].panes[0].job = job;
    let home = home(&layout);
    serde_json::to_value(&home.workspaces[0].terminals[0].panes[0]).unwrap()
}

#[test]
fn a_running_job_sends_its_command() {
    let wire = wire_pane(Some(Job::Running {
        command: "cargo test".into(),
    }));
    assert_eq!(
        wire["job"],
        serde_json::json!({ "state": "running", "command": "cargo test" })
    );
}

#[test]
fn an_idle_shell_says_so() {
    assert_eq!(
        wire_pane(Some(Job::Idle))["job"],
        serde_json::json!({ "state": "idle" })
    );
}

#[test]
fn an_unknown_job_is_left_out() {
    let wire = wire_pane(None);
    assert!(wire.get("job").is_none(), "{wire}");
}

#[test]
fn an_older_server_without_a_job_still_decodes() {
    let pane: model::Pane = serde_json::from_value(serde_json::json!({
        "pane": "p2", "label": null, "cwd": "/work", "focused": true
    }))
    .unwrap();
    assert_eq!(pane.job, None);
}
