//! Settings > Remote control's Share GitHub sign-in switch, one per device:
//! a click sends the device's new choice to the daemon at once.
use super::*;
use crate::{
    config::RemoteConfig,
    remote::{Health, Supervisor},
    settings_window::tests::remote_control::{click, draw, page, status},
};
use sheprd_remote_control::control::{self, Device, DeviceId, Request, Response};
use std::path::Path;

static SHARED: Mutex<Vec<Request>> = Mutex::new(Vec::new());

fn daemon(_: &Path, request: &Request) -> Result<Response, control::Error> {
    if matches!(request, Request::ShareGitHub { .. }) {
        SHARED.lock().unwrap().push(request.clone());
    }
    Ok(Response::Done)
}

fn phone(id: &str, share_github: bool) -> Device {
    Device {
        id: DeviceId(id.into()),
        name: id.into(),
        created_ms: 0,
        last_seen_ms: None,
        push: false,
        connected: false,
        via: None,
        hosts: Vec::new(),
        share_github,
    }
}

#[gpui::test]
fn each_devices_switch_sends_its_new_choice(cx: &mut TestAppContext) {
    let (_, cx, _) = page(cx);
    cx.update(|_, cx| {
        let config = RemoteConfig {
            enabled: true,
            ..RemoteConfig::default()
        };
        Supervisor::fixture(cx, config, daemon);
        Supervisor::set_health(
            cx,
            Health::Running(status()),
            Some(vec![phone("on", true), phone("off", false)]),
        );
    });
    draw(cx);
    click(cx, "remote-device-0-github");
    click(cx, "remote-device-1-github");
    assert_eq!(
        *SHARED.lock().unwrap(),
        [
            Request::ShareGitHub {
                device: DeviceId("on".into()),
                share: false,
            },
            Request::ShareGitHub {
                device: DeviceId("off".into()),
                share: true,
            },
        ]
    );
}
