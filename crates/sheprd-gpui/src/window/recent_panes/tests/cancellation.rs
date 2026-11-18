use super::*;

#[gpui::test]
fn clearing_an_activation_cannot_confirm_the_requested_pane(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    tap(cx, Modifiers::control(), "ctrl-tab");
    view.update(cx, |view, cx| {
        view.live.activation = None;
        view.observe_recent_panes(cx);
        assert!(!view.recent_panes.pending());
        assert_eq!(
            view.recent_panes
                .peek(Step::Back, &view.projections(), Some(&key(LOCAL, "d"))),
            Some((1, key(LOCAL, "c")))
        );
    });
    assert_eq!(focus_requests(&mut peer), ["c"]);
}

#[gpui::test]
fn a_different_completed_activation_does_not_confirm_an_old_request(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    tap(cx, Modifiers::control(), "ctrl-tab");
    view.update(cx, |view, cx| {
        let activation = view.live.activation.as_mut().unwrap();
        activation.request = "a-different-navigation".into();
        activation.revision = Some(7);
        activation.focus = None;
        view.observe_recent_panes(cx);
        assert!(!view.recent_panes.pending());
        assert_eq!(
            view.recent_panes
                .peek(Step::Back, &view.projections(), Some(&key(LOCAL, "d"))),
            Some((1, key(LOCAL, "c")))
        );
    });
    assert_eq!(focus_requests(&mut peer), ["c"]);
}

#[gpui::test]
fn resetting_the_connection_cancels_an_unobserved_selection(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    tap(cx, Modifiers::control(), "ctrl-tab");
    view.update(cx, |view, cx| {
        view.reset_selected(cx);
        assert!(!view.recent_panes.pending());
        assert_eq!(
            view.recent_panes
                .peek(Step::Back, &view.projections(), Some(&key(LOCAL, "d"))),
            Some((1, key(LOCAL, "c")))
        );
    });
    assert_eq!(focus_requests(&mut peer), ["c"]);
}
