use super::*;
use sheprd_remote_control::control::{DeviceId, host_relay::Keys};

fn request() -> Request {
    Request {
        fingerprint: "AB".repeat(32),
        device: DeviceId("far".into()),
        keys: Keys {
            dial: "DIAL".into(),
            tunnel: "TUN".into(),
        },
        signed_ms: 1,
        signature: "SIG".into(),
    }
}

#[test]
fn the_script_finds_the_program_without_installing_it() {
    assert!(SCRIPT.contains("relay-enroll"));
    assert!(SCRIPT.contains("command -v sheprd-remote-control"));
    assert!(SCRIPT.contains("cat >/dev/null"));
    assert!(!SCRIPT.contains("enroll\n"));
}

#[test]
fn a_host_that_prints_nothing_or_too_much_did_not_answer() {
    assert_eq!(parse(b""), None);
    assert_eq!(parse(b" \n"), None);
    assert_eq!(parse(b"usage"), None);
    // A claim without the far computer's signature does not read.
    assert_eq!(parse(br#"{"outcome":"kept"}"#), None);
    assert_eq!(
        parse(br#"{"outcome":"elsewhere"}"#),
        Some(Outcome::Elsewhere)
    );
    // Relays with none named are not an answer.
    assert_eq!(
        parse(
            br#"{"outcome":"relayed","identity":"i","relays":[],"route":"r","proof":{"certificate":"c","key":"k","signature":"s"}}"#
        ),
        None
    );
}

#[test]
fn with_no_host_to_ask_the_computer_is_elsewhere() {
    let outcome = carry(&request(), &[], &AtomicBool::new(false));
    assert_eq!(outcome, Outcome::Elsewhere);
}

#[test]
fn a_cancelled_job_asks_no_host_and_fails_so_the_phone_retries() {
    let target = ConnectTarget::Ssh {
        target: "unreachable.invalid".into(),
        session: "default".into(),
    };
    let outcome = carry(&request(), [&target], &AtomicBool::new(true));
    assert!(matches!(outcome, Outcome::Failed { .. }));
}

#[test]
fn the_search_ends_before_the_near_daemon_stops_waiting() {
    use sheprd_remote_control::control::host_relay::CARRY_MS;
    assert_eq!(BUDGET, Duration::from_millis(CARRY_MS));
}

#[test]
fn an_unsigned_claim_does_not_end_the_search() {
    // Read but not proven: a host claiming the computer without its CA.
    let claim = parse(
        br#"{"outcome":"kept","proof":{"certificate":"Y2VydA","key":"a2V5","signature":"c2ln"}}"#,
    )
    .unwrap();
    assert!(!claim.proven(&request()));
}

fn ssh(target: &str) -> ConnectTarget {
    ConnectTarget::Ssh {
        target: target.into(),
        session: "default".into(),
    }
}

#[test]
fn a_host_out_of_reach_fails_the_search_so_the_phone_retries() {
    let hosts = [ssh("down"), ssh("other")];
    let outcome = search(
        &request(),
        &hosts,
        &AtomicBool::new(false),
        |target| match target {
            ConnectTarget::Ssh { target, .. } if target == "down" => Answer::Unreached,
            _ => Answer::Said(Outcome::Elsewhere),
        },
    );
    assert!(matches!(outcome, Outcome::Failed { .. }));
}

#[test]
fn every_host_asked_and_none_the_computer_is_elsewhere() {
    let hosts = [ssh("old"), ssh("other")];
    let mut asked = Vec::new();
    let outcome = search(&request(), &hosts, &AtomicBool::new(false), |target| {
        asked.push(target.clone());
        match target {
            ConnectTarget::Ssh { target, .. } if target == "old" => Answer::Silent,
            _ => Answer::Said(Outcome::Elsewhere),
        }
    });
    assert_eq!(outcome, Outcome::Elsewhere);
    assert_eq!(asked, hosts);
}

#[test]
fn a_failure_the_computer_reported_is_kept_over_an_unreached_host() {
    let hosts = [ssh("far"), ssh("down")];
    let outcome = search(
        &request(),
        &hosts,
        &AtomicBool::new(false),
        |target| match target {
            ConnectTarget::Ssh { target, .. } if target == "far" => Answer::Said(Outcome::Failed {
                reason: "disk full".into(),
            }),
            _ => Answer::Unreached,
        },
    );
    assert_eq!(
        outcome,
        Outcome::Failed {
            reason: "disk full".into()
        }
    );
}
