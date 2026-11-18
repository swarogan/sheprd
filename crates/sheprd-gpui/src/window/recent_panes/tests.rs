#![allow(clippy::unwrap_used)]
use super::*;
use crate::{
    endpoint::LOCAL,
    keymap::{Binding, DaemonKeys, Keymap},
    palette::Filter,
    sidebar::layout_tests::fixture_window,
    window::MockPeer,
};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;
use sheprd_client::{
    ClientEvent, Method,
    protocol::{ClientShellSnapshot, FrameData, PaneSurfaceFrame},
};
use std::sync::Arc;

mod cancellation;
mod completion;
mod endpoints;
mod legacy_cycle;

const BOOT: &str = "boot-v1";
const PANES: &[&str] = &["a", "b", "c", "d"];

/// The fixture's snapshot with `panes` open and `focused` focused.
fn showing(panes: &[&str], focused: &str) -> ClientShellSnapshot {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))
    .unwrap();
    let pane = snapshot.panes[0].clone();
    snapshot.panes = panes
        .iter()
        .map(|id| {
            let mut pane = pane.clone();
            pane.pane_id = (*id).to_owned();
            pane
        })
        .collect();
    snapshot.focused_pane_id = Some(focused.to_owned());
    snapshot
}

fn snapshot(focused: &str) -> ClientShellSnapshot {
    showing(PANES, focused)
}

fn key(endpoint: &str, pane: &str) -> PaneKey {
    PaneKey::Herdr {
        endpoint: endpoint.into(),
        pane: pane.into(),
    }
}

/// The daemons answer what the window is waiting on: one it switched away
/// from acknowledges letting go of its surface, and the selected one shows
/// `snapshot` on a surface drawn at the window's size, as the window's poll
/// delivers them.
fn arrive(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, snapshot: ClientShellSnapshot) {
    view.update(cx, |view, cx| {
        for endpoint in &view.endpoints {
            let mut inbox = endpoint.connection.inbox.lock().unwrap();
            if let Some(release) = inbox.activation.as_mut().filter(|a| !a.active) {
                release.revision = Some(1);
            }
        }
        // Sends the activation a switch waits for, if any, before it is answered.
        view.poll_endpoints(cx);
        let size = view.options.surface_size;
        let mut state = view.live.clone();
        state.surface = Some(Arc::new(PaneSurfaceFrame {
            boot_id: snapshot.boot_id.clone(),
            projection_revision: snapshot.revision,
            surface_revision: 1,
            frame: FrameData {
                width: size.cols,
                height: size.rows,
                cells: vec![],
                cursor: None,
                hyperlinks: vec![],
                graphics: vec![],
            },
            panes: vec![],
            splits: vec![],
            popup: None,
            graphics: Default::default(),
        }));
        let revision = snapshot.revision;
        state.snapshot = Some(Arc::new(snapshot));
        if let Some(activation) = &mut state.activation {
            activation.revision = Some(revision);
        }
        if state.surface_ready()
            && let Some(activation) = &mut state.activation
        {
            activation.focus = None;
        }
        state.dirty = true;
        *view.endpoints[view.selected_endpoint]
            .connection
            .inbox
            .lock()
            .unwrap() = state;
        view.poll_endpoints(cx);
    });
}

/// The daemon reports `pane` focused.
fn land(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, pane: &str) {
    arrive(view, cx, snapshot(pane));
}

/// A focused window under the app's key bindings, connected to `peer`, that
/// has focused each of `PANES` in turn, so D is focused and C came before it.
fn connected<'a>(
    cx: &'a mut TestAppContext,
    peer: &MockPeer,
    keymap: Keymap,
) -> (Entity<SheprdWindow>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        let mut appearance = crate::app::InitialAppearance::default();
        appearance.config.keybindings = keymap;
        cx.set_global(appearance);
    });
    let (view, cx) = cx.add_window_view(|window, cx| {
        crate::bind_keys(cx);
        fixture_window(window, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            peer.prepare(view, cx);
            view.live.supports_surface = true;
            *view.endpoints[view.selected_endpoint]
                .connection
                .inbox
                .lock()
                .unwrap() = view.live.clone();
            window.focus(&view.focus, cx);
        })
    });
    for pane in PANES {
        land(&view, cx, pane);
    }
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        view.update(cx, |view, _| {
            // The daemon drew the surface at the size this window asks for.
            let size = view.options.surface_size;
            let surface = Arc::make_mut(view.live.surface.as_mut().unwrap());
            (surface.frame.width, surface.frame.height) = (size.cols, size.rows);
        });
        assert!(view.read(cx).input_ready());
    });
    (view, cx)
}

/// The panes the window asked the daemon to focus, in order. The daemon
/// answers one request at a time, so each is answered here. A focus sent from
/// here marks the end, so a request the window never made is missing.
fn focus_requests(peer: &mut MockPeer) -> Vec<String> {
    peer.client.handle.focus_pane(BOOT, "end").unwrap();
    let mut panes = Vec::new();
    loop {
        let request = peer.request();
        let id = request["id"].as_str().unwrap();
        peer.respond(BOOT, id, &json!({"id": id, "result": {"type": "ok"}}));
        if request["method"] != "pane.focus" {
            continue;
        }
        let pane = request["params"]["pane_id"].as_str().unwrap().to_owned();
        if pane == "end" {
            return panes;
        }
        panes.push(pane);
    }
}

fn fenced() -> MockPeer {
    MockPeer::welcoming(
        &["pane.focus", "client_shell.surface.set"],
        &["surface_interest", "presentation_effects_fence"],
    )
}

fn surface_event(
    view: &Entity<SheprdWindow>,
    cx: &VisualTestContext,
    snapshot: &ClientShellSnapshot,
) -> ClientEvent {
    let size = view.read_with(cx, |view, _| view.options.surface_size);
    ClientEvent::Surface(Arc::new(PaneSurfaceFrame {
        boot_id: snapshot.boot_id.clone(),
        projection_revision: snapshot.revision,
        surface_revision: snapshot.revision,
        frame: FrameData {
            width: size.cols,
            height: size.rows,
            cells: vec![],
            cursor: None,
            hyperlinks: vec![],
            graphics: vec![],
        },
        panes: vec![],
        splits: vec![],
        popup: None,
        graphics: Default::default(),
    }))
}

fn publish(
    view: &Entity<SheprdWindow>,
    cx: &mut VisualTestContext,
    events: impl IntoIterator<Item = ClientEvent>,
) {
    view.update(cx, |view, cx| {
        {
            let mut inbox = view.endpoints[view.selected_endpoint]
                .connection
                .inbox
                .lock()
                .unwrap();
            for event in events {
                inbox.apply(event);
            }
        }
        view.poll_endpoints(cx);
    });
}

fn barrier_response(
    peer: &mut MockPeer,
    view: &Entity<SheprdWindow>,
    cx: &VisualTestContext,
    revision: u64,
) -> (ClientEvent, Vec<String>) {
    let expected = view.read_with(cx, |view, _| {
        view.live.activation.as_ref().unwrap().request.clone()
    });
    let mut focused = Vec::new();
    loop {
        let request = peer.request();
        let id = request["id"].as_str().unwrap();
        let result = if request["method"] == Method::ClientShellSurfaceSet.as_str() {
            json!({"type": "client_shell_surface_set", "active": request["params"]["active"], "projection_revision": revision})
        } else {
            if request["method"] == "pane.focus" {
                focused.push(request["params"]["pane_id"].as_str().unwrap().to_owned());
            }
            json!({"type": "ok"})
        };
        let event = peer.respond(BOOT, id, &json!({"id": id, "result": result}));
        if id == expected {
            return (event, focused);
        }
    }
}

fn tap(cx: &mut VisualTestContext, held: Modifiers, keystroke: &str) {
    cx.simulate_modifiers_change(held);
    cx.simulate_keystrokes(keystroke);
    cx.simulate_modifiers_change(Modifiers::none());
}

#[gpui::test]
fn holding_ctrl_walks_back_and_letting_go_settles(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());

    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    land(&view, cx, "c");
    cx.simulate_keystrokes("ctrl-tab");
    land(&view, cx, "b");
    cx.simulate_modifiers_change(Modifiers::none());
    // Settled on B, so a tap goes back to D, the pane before it.
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c", "b", "d"]);
}

#[gpui::test]
fn ctrl_shift_tab_starts_from_the_oldest_pane(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (_, cx) = connected(cx, &peer, Keymap::default());

    tap(cx, Modifiers::control_shift(), "ctrl-shift-tab");

    assert_eq!(focus_requests(&mut peer), ["a"]);
}

#[gpui::test]
fn a_press_while_the_focus_is_landing_is_dropped(cx: &mut TestAppContext) {
    let mut peer = MockPeer::welcoming(
        &["pane.focus", "client_shell.surface.set"],
        &["surface_interest"],
    );
    let (view, cx) = connected(cx, &peer, Keymap::default());
    view.update(cx, |view, _| {
        // Fence focus changes on the surface, as a current daemon does.
        view.live.supports_surface = true;
        let endpoint = &mut view.endpoints[view.selected_endpoint];
        *endpoint.connection.inbox.lock().unwrap() = view.live.clone();
    });

    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    view.read_with(cx, |view, _| {
        assert!(view.live.activation.is_some());
        assert!(!view.input_ready());
    });
    cx.simulate_keystrokes("ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c"]);
    // The cycle is still on C, so the next press goes on to B.
    view.read_with(cx, |view, _| {
        assert!(
            view.recent_panes
                .peek(Step::Back, &view.projections(), Some(&key(LOCAL, "c")))
                .is_none()
        );
    });
}

#[gpui::test]
fn an_open_palette_keeps_ctrl_tab_for_its_filters(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.open_palette(Filter::All, window, cx));
        window.draw(cx).clear(cx);
    });

    tap(cx, Modifiers::control(), "ctrl-tab");

    view.read_with(cx, |view, _| {
        assert_eq!(
            view.menu.palette.as_ref().unwrap().filter,
            Filter::Navigation
        );
    });
    assert!(focus_requests(&mut peer).is_empty());
}

#[gpui::test]
fn a_rebound_shortcut_replaces_ctrl_tab(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let overrides = [("recent_pane".to_owned(), Binding::One("alt-`".into()))].into();
    let keymap =
        Keymap::with_overrides(&overrides, &Default::default(), &DaemonKeys::default()).unwrap();
    let (view, cx) = connected(cx, &peer, keymap);

    cx.simulate_modifiers_change(Modifiers::alt());
    cx.simulate_keystrokes("alt-`");
    land(&view, cx, "c");
    cx.simulate_keystrokes("alt-`");
    cx.simulate_modifiers_change(Modifiers::none());
    tap(cx, Modifiers::control(), "ctrl-tab");

    assert_eq!(focus_requests(&mut peer), ["c", "b"]);
}

#[gpui::test]
fn ime_composition_keeps_the_pane(cx: &mut TestAppContext) {
    let mut peer = fenced();
    let (view, cx) = connected(cx, &peer, Keymap::default());
    view.update(cx, |view, _| view.marked = "か".into());

    tap(cx, Modifiers::control(), "ctrl-tab");

    assert!(focus_requests(&mut peer).is_empty());
}

/// Only a window built by `SheprdWindow::new` watches its activation.
#[cfg(feature = "integration-test")]
#[gpui::test]
fn leaving_the_window_settles_the_cycle(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        SheprdWindow::new(
            sheprd_client::ConnectTarget::Socket("/unused-recent-panes-test.sock".into()),
            window,
            cx,
            true,
        )
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    fn local(snapshot: &ClientShellSnapshot) -> [Projection<'_>; 1] {
        [Projection {
            endpoint: LOCAL,
            available: true,
            snapshot: Some(snapshot),
        }]
    }
    view.update(cx, |view, _| {
        let recent = &mut view.recent_panes;
        for pane in ["a", "b", "c"] {
            recent.observe(&local(&snapshot(pane)), Some(key(LOCAL, pane)));
        }
        let (index, target) = recent
            .peek(Step::Back, &local(&snapshot("c")), Some(&key(LOCAL, "c")))
            .unwrap();
        assert_eq!(target, key(LOCAL, "b"));
        recent.begin(index, Modifiers::control());
    });

    cx.deactivate_window();

    view.update(cx, |view, _| {
        let recent = &mut view.recent_panes;
        recent.observe(&local(&snapshot("b")), Some(key(LOCAL, "b")));
        // Settled on B, so the pane before it is C again.
        assert_eq!(
            recent.peek(Step::Back, &local(&snapshot("b")), Some(&key(LOCAL, "b"))),
            Some((1, key(LOCAL, "c")))
        );
    });
}
