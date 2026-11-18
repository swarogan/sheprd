use super::*;

/// A core chat's keeper, its directory made as the endpoint lock makes it.
fn core(dir: &Path, endpoint: &EndpointId) -> Keeper {
    std::fs::create_dir_all(dir.join("core")).unwrap();
    Keeper::core(
        Some(dir.to_owned()),
        Some(endpoint),
        &builtin(Vec::new()),
        true,
        &abs("/work"),
        |name| match name {
            "HC_TERMINAL_ID" => Some("t9".into()),
            "HC_TERMINAL_SOCKET" => Some(abs("/state/core.sock").into_os_string()),
            _ => None,
        },
    )
    .unwrap()
}

fn recorded_pid(keeper: &Keeper) -> Option<u32> {
    let bytes = std::fs::read(crate::restore::core::started_path(
        &keeper.dir,
        &keeper.endpoint,
    ))
    .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["pid"].as_u64().map(|pid| u32::try_from(pid).unwrap())
}

#[test]
fn a_switch_records_the_agent_that_now_runs() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = ENDPOINT.parse().unwrap();
    let mut keeper = core(dir.path(), &endpoint);
    let running = std::process::id();
    keeper.agent_switched(running);
    assert_eq!(keeper.child, Some(running));
    assert_eq!(recorded_pid(&keeper), Some(running));
}

#[test]
fn a_switch_whose_agent_cannot_be_recorded_forgets_the_one_before() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = ENDPOINT.parse().unwrap();
    let mut keeper = core(dir.path(), &endpoint);
    keeper.agent_started(std::process::id()).unwrap();
    // No process has this id, so its birth cannot be read.
    keeper.agent_switched(u32::MAX);
    assert_eq!(keeper.child, Some(u32::MAX));
    // A later restore finds no agent and refuses, rather than taking the
    // switched-away agent's end for this one's.
    assert_eq!(recorded_pid(&keeper), None);
}

#[test]
fn a_core_chat_whose_recovery_cannot_be_held_refuses_to_switch() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = ENDPOINT.parse().unwrap();
    // No core listens on the keeper's socket.
    let keeper = core(dir.path(), &endpoint);
    assert!(matches!(
        keeper.hold_for_switch(),
        Err(crate::Error::RecoveryHold(_))
    ));
}

#[test]
fn a_herdr_1_chat_has_no_core_recovery_to_hold() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = ENDPOINT.parse().unwrap();
    let keeper = Keeper::new(
        Some(dir.path().to_owned()),
        Some(&target()),
        Some(&endpoint),
        &builtin(Vec::new()),
        false,
        &abs("/work"),
    )
    .unwrap();
    keeper.hold_for_switch().unwrap();
}
