//! The approval step's Share GitHub sign-in switch: on by default, and an
//! approval sent with it off says so.

use super::*;

static LOG: Mutex<Vec<Request>> = Mutex::new(Vec::new());

fn claimed_daemon(_: &Path, request: &Request) -> Result<Response, control::Error> {
    LOG.lock().unwrap().push(request.clone());
    Ok(match request {
        Request::Pair => Response::Offer(offer()),
        Request::Pairing { .. } => Response::Pairing(PairingState::Claimed {
            device_name: "iPhone".into(),
            code: "428913".into(),
            app: None,
            ssh: false,
        }),
        Request::Approve { allow: true, .. } => Response::Pairing(PairingState::Approved {
            device: DeviceId("d1".into()),
        }),
        _ => Response::Done,
    })
}

#[gpui::test]
fn turning_the_switch_off_pairs_without_sharing_github(cx: &mut TestAppContext) {
    LOG.lock().unwrap().clear();
    let (view, cx) = window(cx, true, claimed_daemon);
    open(&view, cx);
    wait(cx);
    assert!(matches!(phase(&view, cx), Phase::Claimed { .. }));
    let sharing = |view: &Entity<SheprdWindow>, cx: &mut VisualTestContext| {
        view.read_with(cx, |view, _| {
            view.menu
                .remote_pairing
                .as_ref()
                .unwrap()
                .pairing
                .share_github()
        })
    };
    assert!(sharing(&view, cx), "on by default");
    draw(cx);
    let switch = cx.debug_bounds("remote-pair-github").unwrap().center();
    cx.simulate_click(switch, Modifiers::default());
    draw(cx);
    assert!(!sharing(&view, cx));
    let allow = cx.debug_bounds("remote-pair-allow").unwrap().center();
    cx.simulate_click(allow, Modifiers::default());
    wait(cx);
    let log = LOG.lock().unwrap().clone();
    assert!(log.contains(&Request::Approve {
        id: PairingId("p1".into()),
        allow: true,
        code: "428913".into(),
        hosts: Vec::new(),
        share_github: false,
    }));
}
