use super::*;

#[gpui::test]
fn legacy_recent_pane_presses_are_explicitly_unsupported(cx: &mut TestAppContext) {
    let mut peer = MockPeer::advertising(&["pane.focus"]);
    let (view, cx) = connected(cx, &peer, Keymap::default());
    view.update(cx, |view, _| {
        view.live.supports_surface = false;
        *view.endpoints[0].connection.inbox.lock().unwrap() = view.live.clone();
    });
    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab ctrl-tab ctrl-tab");
    cx.simulate_modifiers_change(Modifiers::none());
    tap(cx, Modifiers::control(), "ctrl-tab");
    assert!(focus_requests(&mut peer).is_empty());
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.local_error,
            Some(fl!("window-notices-recent-panes-needs-surface"))
        );
        assert!(!view.recent_panes.pending());
    });
}
