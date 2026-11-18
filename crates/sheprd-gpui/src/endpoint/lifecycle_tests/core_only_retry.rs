use super::*;

fn disconnected(view: &mut SheprdWindow, missing: bool) {
    wait_until(|| {
        view.endpoints[0].poll(Instant::now());
        view.endpoints[0].connection.handle.is_none()
    });
    let endpoint = &mut view.endpoints[0];
    {
        let mut state = endpoint.connection.inbox.lock().unwrap();
        state.missing_installation = missing;
        state.apply(ClientEvent::Disconnected {
            reason: "isolated test endpoint".into(),
            ssh: None,
        });
    }
    endpoint.poll(Instant::now());
    endpoint.retry_at = Instant::now();
    view.live = endpoint.live.clone();
}

#[gpui::test]
fn an_absent_herdr_1_does_not_reset_the_core_workspace_on_background_retry(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update(cx, |view, cx| {
        disconnected(view, true);
        let generation = view.endpoints[0].generation;
        for _ in 0..3 {
            view.poll_endpoints(cx);
            assert_eq!(view.endpoints[0].generation, generation);
            assert!(view.endpoints[0].live.missing_installation);
            assert!(view.endpoints[0].connection.handle.is_none());
            assert_eq!(
                view.browser_key().unwrap().1,
                crate::core_terminals::WORKSPACE
            );
        }
        // The user can still check for Herdr 1 after installing it.
        view.reconnect(cx);
        assert!(view.endpoints[0].generation > generation);
    });
}

#[gpui::test]
fn an_unavailable_herdr_1_still_retries_when_it_is_not_known_to_be_absent(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    view.update(cx, |view, cx| {
        disconnected(view, false);
        let generation = view.endpoints[0].generation;
        view.poll_endpoints(cx);
        assert!(view.endpoints[0].generation > generation);
    });
}
