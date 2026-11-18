//! Carrying a phone's relay keys: each request runs once, and its outcome
//! goes back to the daemon.

use super::*;
use sheprd_remote_control::control::host_relay::{self, Keys, Outcome};

fn job() -> host_relay::Job {
    host_relay::Job {
        device: DeviceId("d1".into()),
        host: None,
        request: host_relay::Request {
            fingerprint: "AB".repeat(32),
            device: DeviceId("far".into()),
            keys: Keys {
                dial: "DIAL".into(),
                tunnel: "TUN".into(),
            },
            signed_ms: 1,
            signature: "SIG".into(),
        },
    }
}

fn incoming() -> Incoming {
    Incoming {
        sent: None,
        pairings: Vec::new(),
        relays: vec![job()],
        revokes: None,
    }
}

#[gpui::test]
fn a_relay_request_is_carried_once_and_answered(cx: &mut TestAppContext) {
    SENT.lock().unwrap().clear();
    cx.update(|cx| {
        Supervisor::fixture(cx, on(), daemon);
        Supervisor::take_incoming(cx, incoming());
        Supervisor::take_incoming(cx, incoming());
        assert_eq!(cx.global::<Supervisor>().links.relaying.len(), 1);
    });
    cx.run_until_parked();
    // No host this app connects to is that computer.
    let answers: Vec<Request> = SENT
        .lock()
        .unwrap()
        .iter()
        .filter(|request| matches!(request, Request::HostRelayed { .. }))
        .cloned()
        .collect();
    assert_eq!(
        answers,
        vec![Request::HostRelayed {
            device: DeviceId("d1".into()),
            relay: Box::new(job().request),
            outcome: Box::new(Outcome::Elsewhere),
        }]
    );
    cx.update(|cx| assert!(cx.global::<Supervisor>().links.relaying.is_empty()));
}
