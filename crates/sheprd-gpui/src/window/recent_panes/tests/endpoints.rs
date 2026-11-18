use super::*;
use crate::{endpoint::Endpoint, navigation::NavigationTarget, state::LiveState};
use sheprd_client::ConnectTarget;

const REMOTE: &str = "remote";
const HOSTS: &[&str] = &["x", "y"];

/// Local, through `local`, and a remote device, through `remote`. The window
/// went to the remote's X and came back to Local's D, so the order is D, X,
/// C, B, A.
fn two_machines<'a>(
    cx: &'a mut TestAppContext,
    local: &MockPeer,
    remote: &MockPeer,
) -> (Entity<SheprdWindow>, &'a mut VisualTestContext) {
    let (view, cx) = connected(cx, local, Keymap::default());
    view.update(cx, |view, _| {
        view.live.supports_surface = true;
        let mut device = Endpoint::new(
            REMOTE.into(),
            "Remote".into(),
            ConnectTarget::Ssh {
                target: "recent-panes.invalid".into(),
                session: "default".into(),
            },
            true,
        );
        device.connection.handle = Some(remote.client.handle.clone());
        device.live = view.live.clone();
        device.live.snapshot = Some(Arc::new(showing(HOSTS, "x")));
        device.live.invalidate_surface();
        *device.connection.inbox.lock().unwrap() = device.live.clone();
        view.endpoints.push(device);
    });
    land(&view, cx, "d");
    select(&view, cx, REMOTE);
    arrive(&view, cx, showing(HOSTS, "x"));
    select(&view, cx, LOCAL);
    land(&view, cx, "d");
    (view, cx)
}

/// The user picks `endpoint` in the sidebar.
fn select(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, endpoint: &str) {
    view.update(cx, |view, cx| assert!(view.select_endpoint(endpoint, cx)));
}

/// The remote's daemon sends `change` while the window shows Local.
fn remote_sends(
    view: &Entity<SheprdWindow>,
    cx: &mut VisualTestContext,
    change: impl FnOnce(&mut LiveState),
) {
    view.update(cx, |view, cx| {
        let remote = &view.endpoints[1];
        let mut state = remote.live.clone();
        change(&mut state);
        state.dirty = true;
        *remote.connection.inbox.lock().unwrap() = state;
        view.poll_endpoints(cx);
    });
}

/// The selected endpoint and the navigation waiting for it to activate.
fn switched(
    view: &Entity<SheprdWindow>,
    cx: &mut VisualTestContext,
) -> (String, Option<NavigationTarget<String>>) {
    view.read_with(cx, |view, _| {
        (
            view.endpoints[view.selected_endpoint].id.clone(),
            view.pending_navigation.clone(),
        )
    })
}

fn pane(id: &str) -> Option<NavigationTarget<String>> {
    Some(NavigationTarget::Pane(id.into()))
}

#[gpui::test]
fn a_tap_goes_to_another_machine_and_the_next_comes_back(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    // The remote reconnected while away, so it has no surface to give back,
    // and another client moved its focus to Y.
    remote_sends(&view, cx, |state| {
        state.activation = None;
        state.snapshot = Some(Arc::new(showing(HOSTS, "y")));
    });

    tap(cx, Modifiers::control(), "ctrl-tab");
    assert_eq!(switched(&view, cx), (REMOTE.into(), pane("x")));
    // A poll before Local lets go of its surface shows Y, the remote's focus
    // before the switch, which is not where the user went.
    view.update(cx, |view, cx| view.poll_endpoints(cx));
    arrive(&view, cx, showing(HOSTS, "y"));
    arrive(&view, cx, showing(HOSTS, "x"));
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx), (LOCAL.into(), pane("d")));
    assert_eq!(focus_requests(&mut remote), ["x"]);
    assert!(focus_requests(&mut local).is_empty());
}

#[gpui::test]
fn holding_ctrl_walks_across_machines(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);

    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    arrive(&view, cx, showing(HOSTS, "x"));
    arrive(&view, cx, showing(HOSTS, "x"));
    cx.simulate_keystrokes("ctrl-tab");
    land(&view, cx, "d");
    land(&view, cx, "c");
    cx.simulate_modifiers_change(Modifiers::none());
    // Settled on C, so a tap goes back to D, where the walk began.
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut remote), ["x"]);
    assert_eq!(focus_requests(&mut local), ["c", "d"]);
}

#[gpui::test]
fn a_press_before_the_other_machine_is_ready_is_dropped(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);

    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    cx.simulate_keystrokes("ctrl-tab");

    assert_eq!(switched(&view, cx), (REMOTE.into(), pane("x")));
    arrive(&view, cx, showing(HOSTS, "x"));
    assert_eq!(focus_requests(&mut remote), ["x"]);
    assert!(focus_requests(&mut local).is_empty());
}

#[gpui::test]
fn a_disabled_machine_is_stepped_over(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    view.update(cx, |view, _| view.endpoints[1].enabled = false);

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx).0, LOCAL);
    assert_eq!(focus_requests(&mut local), ["c"]);
    assert!(focus_requests(&mut remote).is_empty());
}

#[gpui::test]
fn a_restarted_machine_forgets_only_its_own_panes(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    remote_sends(&view, cx, |state| {
        let mut snapshot = showing(HOSTS, "x");
        snapshot.boot_id = "boot-v2".into();
        state.snapshot = Some(Arc::new(snapshot));
    });

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(switched(&view, cx).0, LOCAL);
    assert_eq!(focus_requests(&mut local), ["c"]);
    assert!(focus_requests(&mut remote).is_empty());
}

mod cancellation;
mod connectivity;
mod legacy;
