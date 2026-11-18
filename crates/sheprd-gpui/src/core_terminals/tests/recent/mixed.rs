use super::*;
use crate::{SheprdWindow, window::MockPeer};
use gpui::{Entity, VisualTestContext};
use serde_json::json;
use sheprd_client::protocol::{FrameData, PaneSurfaceFrame};
use std::sync::Arc;

fn mixed(
    cx: &mut TestAppContext,
) -> (
    Entity<SheprdWindow>,
    &mut VisualTestContext,
    crate::browser::TabId,
    MockPeer,
) {
    let peer = MockPeer::welcoming(
        &["pane.focus", "workspace.focus", "client_shell.surface.set"],
        &["surface_interest", "presentation_effects_fence"],
    );
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let target = view.endpoints[0].connection.target.clone();
            peer.prepare(view, cx);
            view.endpoints[0].connection.target = target;
            view.live.supports_surface = true;
            view.endpoints[0].request_surface_for_test();
            view.endpoints[0].live = view.live.clone();
            *view.endpoints[0].connection.inbox.lock().unwrap() = view.live.clone();
            window.focus(&view.focus, cx);
            view.observe_recent_focus(window, cx);
            view.show_core_pane(tab, &id("t1"), window, cx);
            view.observe_recent_focus(window, cx);
            assert!(view.input_ready());
        })
    });
    (view, cx, tab, peer)
}

fn finish(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let snapshot = view.live.snapshot.as_ref().unwrap();
            let revision = snapshot.revision;
            let size = view.options.surface_size;
            view.live.surface = Some(Arc::new(PaneSurfaceFrame {
                boot_id: snapshot.boot_id.clone(),
                projection_revision: revision,
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
            let activation = view.live.activation.as_mut().unwrap();
            activation.revision = Some(revision);
            activation.focus = None;
            assert!(view.input_ready());
            *view.endpoints[0].connection.inbox.lock().unwrap() = view.live.clone();
            view.observe_recent_focus(window, cx);
        })
    });
}

fn request(peer: &mut MockPeer) -> serde_json::Value {
    let request = peer.request();
    let id = request["id"].as_str().unwrap();
    peer.respond("boot-v1", id, &json!({"id": id, "result": {"type":"ok"}}));
    request
}

#[gpui::test]
fn core_and_herdr_panes_share_a_toggle_and_preserve_keyboard_focus(cx: &mut TestAppContext) {
    let (view, cx, tab, mut peer) = mixed(cx);
    run(&view, cx, Command::RecentPane);
    let focus = request(&mut peer);
    assert_eq!(focus["method"], "pane.focus");
    assert_eq!(focus["params"]["pane_id"], "w1:p1");
    finish(&view, cx);
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        let view = view.read(cx);
        assert_eq!(view.core_command_pane(window, cx), Some((tab, id("t1"))));
        assert_eq!(
            view.group_shown(view.active_group().unwrap(), cx),
            Shown::Page(tab)
        );
    });
}

fn second_workspace(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let snapshot = Arc::make_mut(view.live.snapshot.as_mut().unwrap());
            let mut workspace = snapshot.workspaces[0].clone();
            workspace.workspace_id = "w2".into();
            workspace.active_tab_id = "w2:t2".into();
            snapshot.workspaces.push(workspace);
            let mut tab = snapshot.tabs[0].clone();
            tab.tab_id = "w2:t2".into();
            tab.workspace_id = "w2".into();
            snapshot.tabs.push(tab);
            let mut pane = snapshot.panes[0].clone();
            pane.pane_id = "w2:p2".into();
            pane.tab_id = "w2:t2".into();
            pane.workspace_id = "w2".into();
            snapshot.panes.push(pane);
            let group = view.active_group().unwrap();
            view.ensure_layout().unwrap().follow_terminal(group);
            window.focus(&view.focus, cx);
            view.observe_recent_focus(window, cx);
            let snapshot = Arc::make_mut(view.live.snapshot.as_mut().unwrap());
            snapshot.focused_workspace_id = Some("w2".into());
            snapshot.focused_tab_id = Some("w2:t2".into());
            snapshot.focused_pane_id = Some("w2:p2".into());
            view.observe_recent_focus(window, cx);
        })
    });
}

#[gpui::test]
fn coalesced_workspace_completion_cannot_confirm_a_core_pane_never_shown(cx: &mut TestAppContext) {
    let (view, cx, tab, mut peer) = mixed(cx);
    second_workspace(&view, cx);
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        view.update(cx, |view, _| {
            let size = view.options.surface_size;
            let surface = Arc::make_mut(view.live.surface.as_mut().unwrap());
            (surface.frame.width, surface.frame.height) = (size.cols, size.rows);
            *view.endpoints[0].connection.inbox.lock().unwrap() = view.live.clone();
        });
        let view = view.read(cx);
        assert!(
            view.input_ready(),
            "the second workspace has a coherent surface"
        );
        assert_eq!(view.browser_key().unwrap().1, "w2");
        assert!(view.recent_core_panes(cx).contains(&(
            crate::core_terminals::recent::RecentCorePane::Tab {
                tab,
                terminal: id("t1"),
            },
            true,
        )));
    });
    run(&view, cx, Command::RecentPaneReverse);
    let workspace = request(&mut peer);
    assert_eq!(workspace["method"], "workspace.focus");
    assert_eq!(workspace["params"]["workspace_id"], "w1");
    finish(&view, cx);
    cx.update(|window, cx| {
        assert_eq!(view.read(cx).browser_key().unwrap().1, "w2");
        assert_eq!(view.read(cx).core_command_pane(window, cx), None);
    });
    run(&view, cx, Command::RecentPane);
    assert_eq!(request(&mut peer)["method"], "client_shell.surface.set");
    let focus = request(&mut peer);
    assert_eq!(focus["method"], "pane.focus");
    assert_eq!(focus["params"]["pane_id"], "w1:p1");
}

#[gpui::test]
fn an_inaccessible_core_workspace_is_skipped_for_a_reachable_mru_pane(cx: &mut TestAppContext) {
    let (view, cx, tab, mut peer) = mixed(cx);
    run(&view, cx, Command::FocusRight);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.observe_recent_panes(cx);
            assert!(Store::update(cx, |store| store.move_to_workspace(tab, "gone-workspace")));
        })
    });
    run(&view, cx, Command::RecentPane);
    let focus = request(&mut peer);
    assert_eq!(focus["method"], "pane.focus");
    assert_eq!(focus["params"]["pane_id"], "w1:p1");
}
