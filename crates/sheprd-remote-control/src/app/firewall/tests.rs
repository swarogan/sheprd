use super::*;

#[test]
fn firewall_state_is_read_from_its_number() {
    assert!(firewall_on("Firewall is enabled. (State = 1)\n"));
    assert!(firewall_on(
        "Firewall is blocking all non-essential. (State = 2)"
    ));
    assert!(!firewall_on("Firewall is disabled. (State = 0)\n"));
}

#[test]
fn unknown_firewall_wording_is_taken_as_off() {
    assert!(!firewall_on(""));
    assert!(!firewall_on("Firewall is enabled."));
    assert!(!firewall_on("(State = on)"));
    assert!(!firewall_on("(State = 1"));
}

#[test]
fn a_developer_id_signature_has_a_team() {
    let text = "Executable=/Applications/Herdr.app/Contents/MacOS/sheprd-remote-control\n\
                Authority=Developer ID Application: Someone (VJAHQVZ96V)\n\
                TeamIdentifier=VJAHQVZ96V\n";
    assert_eq!(signer(true, text), Some(Signer::Team));
}

#[test]
fn ad_hoc_and_unsigned_builds_have_nobody() {
    let ad_hoc = "CodeDirectory v=20400 size=168584 flags=0x20002(adhoc,linker-signed)\n\
                  Signature=adhoc\n\
                  TeamIdentifier=not set\n";
    assert_eq!(signer(true, ad_hoc), Some(Signer::Nobody));
    // codesign exits 1 for an unsigned file, and says why.
    let unsigned = "sheprd-remote-control: code object is not signed at all\n";
    assert_eq!(signer(false, unsigned), Some(Signer::Nobody));
}

#[test]
fn a_failed_or_unfamiliar_answer_names_no_signer() {
    // A missing or unreadable file, or something that is not code.
    assert_eq!(
        signer(false, "sheprd-remote-control: No such file or directory\n"),
        None
    );
    assert_eq!(signer(false, "TeamIdentifier=not set\n"), None);
    assert_eq!(signer(true, ""), None);
    assert_eq!(signer(true, "Signature=adhoc\n"), None);
    assert_eq!(signer(true, "TeamIdentifier=\n"), None);
}

#[cfg(unix)]
#[test]
fn output_reads_both_streams_and_the_status() {
    let (status, text) = output(
        "/bin/sh",
        ["-c".as_ref(), "echo out; echo err >&2; exit 3".as_ref()],
        DEADLINE,
    )
    .unwrap();
    assert_eq!(status.code(), Some(3));
    assert!(text.contains("out") && text.contains("err"), "{text}");
}

#[cfg(unix)]
#[test]
fn output_gives_up_on_a_command_past_its_deadline() {
    let started = std::time::Instant::now();
    let deadline = Duration::from_millis(200);
    let error = output("/bin/sleep", ["30".as_ref()], deadline).unwrap_err();
    assert!(matches!(
        error,
        Error::Timeout {
            program: "/bin/sleep"
        }
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[cfg(unix)]
#[test]
fn output_gives_up_on_a_command_that_closes_its_streams_but_keeps_running() {
    let started = std::time::Instant::now();
    let deadline = Duration::from_millis(200);
    let error = output(
        "/bin/sh",
        ["-c".as_ref(), "exec >/dev/null 2>&1; sleep 30".as_ref()],
        deadline,
    )
    .unwrap_err();
    assert!(matches!(error, Error::Timeout { program: "/bin/sh" }));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_missing_command_fails_to_spawn() {
    let error = output("/nowhere/socketfilterfw", std::iter::empty(), DEADLINE).unwrap_err();
    assert!(matches!(error, Error::Spawn { .. }));
    assert!(std::error::Error::source(&error).is_some());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn approval_lasts_without_a_macos_firewall() {
    assert_eq!(
        approval(Path::new("/nowhere/sheprd-remote-control")).unwrap(),
        Approval::Lasting
    );
}
