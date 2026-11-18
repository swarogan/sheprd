use super::*;
use crate::state::ConnectionStatus;
use sheprd_client::{
    ClientEvent,
    protocol::{ClientMessage, ClientPaneInputEvent},
};

#[gpui::test]
fn a_dead_background_transport_with_a_retained_snapshot_is_skipped(cx: &mut TestAppContext) {
    let (mut local, remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    remote.client.handle.disconnect();
    assert!(view.read_with(cx, |view, _| view.endpoints[1].live.snapshot.is_some()));

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx).0, LOCAL);
    assert_eq!(focus_requests(&mut local), ["c"]);
}

#[gpui::test]
fn a_background_reconnection_to_the_same_boot_keeps_its_history(cx: &mut TestAppContext) {
    let (local, remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    remote.client.handle.disconnect();
    view.update(cx, |view, cx| view.poll_endpoints(cx));
    let replacement = fenced();
    view.update(cx, |view, cx| {
        view.endpoints[1].connection.handle = Some(replacement.client.handle.clone());
        view.endpoints[1].live.status = ConnectionStatus::Connected;
        view.observe_recent_panes(cx);
    });

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx), (REMOTE.into(), pane("x")));
}

#[gpui::test]
fn an_initial_live_connection_cannot_escape_its_activation_fence(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    view.update(cx, |view, _| {
        view.live.status = ConnectionStatus::Connecting;
        view.live.surface = None;
        assert!(view.endpoints[0].outage().is_none());
        assert!(
            !view.endpoints[0]
                .connection
                .handle
                .as_ref()
                .unwrap()
                .is_disconnected()
        );
    });

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx).0, LOCAL);
    assert!(focus_requests(&mut local).is_empty());
    assert!(focus_requests(&mut remote).is_empty());
}

#[gpui::test]
fn a_selected_outage_escapes_without_replaying_held_input_or_old_navigation(
    cx: &mut TestAppContext,
) {
    let (mut local, remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    select(&view, cx, REMOTE);
    arrive(&view, cx, showing(HOSTS, "x"));
    view.update(cx, |view, cx| {
        assert!(view.input_ready());
        view.flush_pending_input(cx);
        view.live.surface = None;
        *view.endpoints[1].connection.inbox.lock().unwrap() = view.live.clone();
        view.send(
            ClientPaneInputEvent::TextCommit("held on remote".into()),
            cx,
        );
        assert_eq!(view.pending_input.len(), 1);
        view.pending_navigation = pane("y");
    });
    remote.client.handle.disconnect();
    tap(cx, Modifiers::control(), "ctrl-tab");
    view.read_with(cx, |view, _| {
        assert_eq!(view.endpoints[view.selected_endpoint].id, LOCAL);
        assert_eq!(view.pending_input.len(), 0);
        assert_ne!(view.pending_navigation, pane("y"));
    });
    view.update(cx, |view, cx| view.poll_endpoints(cx));
    let mut target = snapshot("d");
    target.revision += 1;
    let (ack, focused) = barrier_response(&mut local, &view, cx, target.revision);
    assert!(focused.is_empty());
    let frame = surface_event(&view, cx, &target);
    publish(
        &view,
        cx,
        [ack, ClientEvent::Snapshot(Arc::new(target.clone())), frame],
    );
    let (ack, focused) = barrier_response(&mut local, &view, cx, target.revision);
    assert_eq!(focused, ["d"]);
    let frame = surface_event(&view, cx, &target);
    publish(&view, cx, [ack, frame]);
    view.update(cx, |view, _| {
        assert!(view.input_ready());
        assert!(view.pending_navigation.is_none());
        view.endpoints[0]
            .connection
            .handle
            .as_ref()
            .unwrap()
            .set_focus(BOOT, false)
            .unwrap();
    });
    loop {
        match local.receive() {
            ClientMessage::ClientShellFocus { focused: false } => break,
            ClientMessage::ClientShellPaneInput { .. } => panic!("held remote input reached Local"),
            ClientMessage::ClientShellEndpointRequest { request, .. } => {
                let request: serde_json::Value = serde_json::from_str(&request).unwrap();
                assert_ne!(
                    request["method"], "pane.focus",
                    "old navigation reached Local"
                );
            }
            _ => {}
        }
    }
}

#[gpui::test]
fn a_dead_local_legacy_connection_can_escape_to_a_healthy_remote(cx: &mut TestAppContext) {
    let (local, remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    view.update(cx, |view, _| {
        view.live.supports_surface = false;
        view.endpoints[0].live.supports_surface = false;
    });
    local.client.handle.disconnect();

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx), (REMOTE.into(), pane("x")));
}

#[gpui::test]
fn a_live_local_legacy_connection_keeps_surface_switching_refused(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    view.update(cx, |view, _| {
        view.live.supports_surface = false;
        view.endpoints[0].live.supports_surface = false;
    });

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx), (LOCAL.into(), None));
    assert!(focus_requests(&mut local).is_empty());
    assert!(focus_requests(&mut remote).is_empty());
}
