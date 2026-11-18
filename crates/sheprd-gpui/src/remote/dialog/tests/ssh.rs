//! Another computer's Sheprd asking to open terminals here: the approval
//! says what Allow does, and leaves out what only a phone is asked.

use super::*;

static LOG: Mutex<Vec<Request>> = Mutex::new(Vec::new());

fn desktop_daemon(_: &Path, request: &Request) -> Result<Response, control::Error> {
    LOG.lock().unwrap().push(request.clone());
    Ok(match request {
        Request::Pair => Response::Offer(offer()),
        Request::Pairing { .. } => Response::Pairing(PairingState::Claimed {
            device_name: "Laptop".into(),
            code: "428913".into(),
            app: None,
            ssh: true,
        }),
        Request::Approve { allow: true, .. } => Response::Pairing(PairingState::Approved {
            device: DeviceId("d1".into()),
        }),
        _ => Response::Done,
    })
}

#[gpui::test]
fn a_computer_asking_for_ssh_is_named_as_one_and_allowed(cx: &mut TestAppContext) {
    LOG.lock().unwrap().clear();
    let (view, cx) = window(cx, true, desktop_daemon);
    open(&view, cx);
    wait(cx);
    assert!(matches!(phase(&view, cx), Phase::Claimed { ssh: true, .. }));
    draw(cx);
    assert!(cx.debug_bounds("remote-pair-ssh").is_some());
    assert!(cx.debug_bounds("remote-pair-github").is_none());
    let allow = cx.debug_bounds("remote-pair-allow").unwrap().center();
    cx.simulate_click(allow, Modifiers::default());
    wait(cx);
    assert!(matches!(
        phase(&view, cx),
        Phase::Approved { ssh: true, .. }
    ));
    assert!(LOG.lock().unwrap().iter().any(|request| matches!(
        request,
        Request::Approve { allow: true, code, .. } if code == "428913"
    )));
}
