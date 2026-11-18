use super::*;

#[gpui::test]
fn replacing_deferred_navigation_cannot_confirm_the_old_destination(cx: &mut TestAppContext) {
    let (local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    tap(cx, Modifiers::control(), "ctrl-tab");
    assert_eq!(switched(&view, cx), (REMOTE.into(), pane("x")));
    view.update(cx, |view, cx| {
        for endpoint in &view.endpoints {
            let mut inbox = endpoint.connection.inbox.lock().unwrap();
            if let Some(release) = inbox
                .activation
                .as_mut()
                .filter(|activation| !activation.active)
            {
                release.revision = Some(1);
            }
        }
        view.poll_endpoints(cx);
        assert!(view.live.activation.is_some());
        assert!(view.recent_panes.pending());
    });
    let snapshot = showing(HOSTS, "y");
    let frame = surface_event(&view, cx, &snapshot);
    view.update(cx, |view, cx| {
        let mut state = view.live.clone();
        state.activation.as_mut().unwrap().revision = Some(snapshot.revision);
        state.apply(ClientEvent::Snapshot(Arc::new(snapshot)));
        state.apply(frame);
        *view.endpoints[view.selected_endpoint]
            .connection
            .inbox
            .lock()
            .unwrap() = state;
        view.pending_navigation = pane("y");
        view.poll_endpoints(cx);
        assert!(!view.recent_panes.pending());
        assert!(view.recent_panes.selected().is_none());
    });
    assert_eq!(focus_requests(&mut remote), ["y"]);
}
