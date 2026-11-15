//! Running a GPU tool on this machine: its answer, and the failures that
//! must not hold up the load worker.
use super::*;
use crate::system_load::gpu::{Probe, TOOL_LIMIT, Unanswered, tool};

fn sh(script: &str, timeout: Duration) -> Option<String> {
    tool("/bin/sh", ["-c", script], timeout).ok()
}

#[test]
fn a_tool_that_exits_cleanly_is_read() {
    let limit = Duration::from_secs(10);
    assert_eq!(sh("echo 45", limit).as_deref(), Some("45\n"));
    assert_eq!(sh("echo 45; exit 1", limit), None, "a failure is no answer");
}

/// Only a tool that is not installed is given up on; one that fails or is
/// slow while the machine is busy is run again on the next sample.
#[test]
fn only_a_missing_tool_is_missing() {
    let limit = Duration::from_secs(10);
    assert_eq!(
        tool("herdr-no-such-gpu-tool", [], limit),
        Err(Unanswered::Missing)
    );
    assert_eq!(
        tool("/bin/sh", ["-c", "exit 1"], limit),
        Err(Unanswered::Failed)
    );
    assert_eq!(
        tool("/bin/sh", ["-c", "sleep 30"], Duration::from_millis(300)),
        Err(Unanswered::Failed)
    );
}

#[test]
fn a_tool_that_closes_its_output_and_keeps_running_is_ended() {
    let started = Instant::now();
    assert_eq!(sh("exec >&-; sleep 30", Duration::from_millis(300)), None);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_tool_that_keeps_printing_is_ended() {
    let started = Instant::now();
    assert_eq!(
        sh(
            "while :; do echo 1; sleep 0.05; done",
            Duration::from_millis(300)
        ),
        None
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn an_answer_past_the_cap_is_no_answer() {
    let limit = Duration::from_secs(10);
    let full = sh(&format!("head -c {TOOL_LIMIT} /dev/zero"), limit);
    assert_eq!(full.map(|text| text.len()), Some(TOOL_LIMIT));
    assert_eq!(
        sh(&format!("head -c {} /dev/zero", TOOL_LIMIT + 1), limit),
        None
    );
}

/// The bug this guards against: one slow or failed answer used to stop the
/// tool for good, so this machine's GPU usage vanished until a restart.
#[test]
fn a_tool_that_failed_once_answers_on_the_next_sample() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("failed-once");
    // Fails the first time it runs, then answers.
    let script = format!(
        "if [ -e '{0}' ]; then echo 45; else : > '{0}'; exit 1; fi",
        marker.display()
    );
    let args = ["-c", script.as_str()];
    let mut probe = Probe::new("/bin/sh", &args);
    let limit = Duration::from_secs(10);
    assert_eq!(probe.read(limit), None);
    assert!(marker.exists(), "the first sample ran the tool");
    assert_eq!(probe.read(limit).as_deref(), Some("45\n"));
}

#[test]
fn a_missing_tool_is_not_run_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("gpu-tool");
    let program = program.to_str().unwrap();
    let limit = Duration::from_secs(10);
    let mut probe = Probe::new(program, &[]);
    assert_eq!(probe.read(limit), None);
    // Installed afterwards, it is still not run by the probe that missed it.
    std::fs::write(program, "#!/bin/sh\necho 45\n").unwrap();
    std::fs::set_permissions(program, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(probe.read(limit), None);
    assert_eq!(
        Probe::new(program, &[]).read(limit).as_deref(),
        Some("45\n"),
        "a new worker finds it"
    );
}
