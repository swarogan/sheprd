use super::*;
use gpui::VisualTestContext;

/// A remote host named after its id, `ssh:alpha-box` as `alpha-box`.
fn remote(id: &str, view: &SheprdWindow) -> crate::endpoint::Endpoint {
    let mut remote = crate::endpoint::Endpoint::new(
        id.into(),
        id.trim_start_matches("ssh:").into(),
        ConnectTarget::Ssh {
            target: "example.invalid".into(),
            session: "default".into(),
        },
        true,
    );
    remote.live.snapshot = view.live.snapshot.clone();
    remote
}

/// A window with Local and `remotes` more hosts, drawn.
fn open<'a>(
    cx: &'a mut gpui::TestAppContext,
    remotes: &[&str],
) -> (Entity<SheprdWindow>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut view = fixture_window(window, cx);
        for id in remotes {
            let endpoint = remote(id, &view);
            view.endpoints.push(endpoint);
        }
        view
    });
    cx.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    (view, cx)
}

fn collapsed(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> Vec<bool> {
    view.read_with(cx, |view, _| {
        view.endpoints.iter().map(|e| e.collapsed).collect()
    })
}

fn click(cx: &mut VisualTestContext, selector: &'static str, modifiers: Modifiers) {
    let bounds = cx.debug_bounds(selector).unwrap();
    cx.simulate_click(bounds.center(), modifiers);
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
}

#[gpui::test]
fn the_heading_button_collapses_every_host_then_expands_them(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, &["ssh:a", "ssh:b"]);
    let heading = cx.debug_bounds("header-spaces").unwrap();
    let button = cx.debug_bounds("sidebar-fold-hosts").unwrap();
    assert!(
        heading.right() <= button.left(),
        "the button follows the heading"
    );
    click(cx, "sidebar-fold-hosts", Modifiers::default());
    assert_eq!(collapsed(&view, cx), [true; 3]);
    assert!(cx.debug_bounds("host-ssh:b").is_some(), "headers stay");
    click(cx, "sidebar-fold-hosts", Modifiers::default());
    assert_eq!(collapsed(&view, cx), [false; 3]);
}

#[gpui::test]
fn a_partly_collapsed_list_collapses_the_rest_first(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, &["ssh:a"]);
    click(cx, "collapse-host-local", Modifiers::default());
    assert_eq!(collapsed(&view, cx), [true, false]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.command(crate::controls::Command::ToggleAllHosts, window, cx);
        });
    });
    assert_eq!(collapsed(&view, cx), [true, true]);
}

#[gpui::test]
fn alt_clicking_a_host_arrow_folds_every_host(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, &["ssh:a", "ssh:b"]);
    click(cx, "collapse-host-local", Modifiers::alt());
    assert_eq!(collapsed(&view, cx), [true; 3]);
}

#[gpui::test]
fn a_single_host_offers_no_button(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, &[]);
    assert!(cx.debug_bounds("sidebar-fold-hosts").is_none());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.command(crate::controls::Command::ToggleAllHosts, window, cx);
        });
    });
    assert_eq!(collapsed(&view, cx), [false]);
}

#[gpui::test]
fn the_devices_search_limits_which_hosts_fold(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, &["ssh:alpha-box", "ssh:beta-box"]);
    let input = cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.config.layout.mode = crate::config::LayoutMode::Devices;
            view.endpoints[1].collapsed = true;
            view.endpoints[2].collapsed = true;
            view.poll_devices_overview(cx);
            view.devices_overview
                .tree_search
                .as_ref()
                .unwrap()
                .input
                .clone()
        })
    });
    cx.update(|window, cx| window.focus(&input.read(cx).focus.clone(), cx));
    cx.simulate_input("box");
    cx.run_until_parked();
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    assert!(
        cx.debug_bounds("host-local").is_none(),
        "Local is searched away"
    );
    // Both hosts on screen are folded, so the button expands them and leaves
    // the hidden, open Local alone.
    let folded = cx.update(|_, cx| view.read(cx).hosts_folded(cx));
    assert_eq!(folded, Some(true));
    click(cx, "sidebar-fold-hosts", Modifiers::default());
    assert_eq!(collapsed(&view, cx), [false, false, false]);
}

#[gpui::test]
fn the_button_is_a_keyboard_stop_in_the_sidebar(cx: &mut gpui::TestAppContext) {
    let (view, cx) = open(cx, &["ssh:a"]);
    cx.update(|window, _| window.activate_window());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            window.focus(&view.sidebar_search.focus(cx), cx);
            view.chrome_region = Some(crate::window::chrome_focus::Region::Sidebar);
        });
        full_draw(window, cx).clear(cx);
    });
    // The heading's field, then its button, which Enter presses.
    cx.update(|window, cx| {
        window.focus_next(cx);
        full_draw(window, cx).clear(cx);
    });
    let enter = gpui::Keystroke::parse("enter").unwrap();
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: enter.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent { keystroke: enter });
    cx.update(|window, cx| full_draw(window, cx).clear(cx));
    assert_eq!(collapsed(&view, cx), [true, true]);
}
