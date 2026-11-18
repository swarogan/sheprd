use super::commands::{id, run, window_with_split};
use crate::{
    browser::{Shown, Store},
    controls::Command,
    core_terminals::{WORKSPACE, fallback::Fallback},
};
use gpui::{Modifiers, TestAppContext};

mod mixed;
mod shell;

#[gpui::test]
fn core_only_recent_commands_toggle_without_an_intervening_poll(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.live.snapshot = None;
            view.live.missing_installation = true;
            view.refresh_core_session(cx);
            assert!(Store::update(cx, |store| store.move_to_workspace(tab, WORKSPACE)));
            view.show_core_pane(tab, &id("t1"), window, cx);
        })
    });
    run(&view, cx, Command::FocusRight);
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t1")))
        );
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t2")))
        );
        assert!(view.read(cx).live.snapshot.is_none());
    });
}

#[gpui::test]
fn local_recent_panes_ignore_an_unrelated_herdr_activation(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_core_pane(tab, &id("t1"), window, cx);
            view.live.activation = Some(crate::state::SurfaceActivation {
                request: "unrelated-herdr-startup".into(),
                boot: "boot-v1".into(),
                revision: None,
                failed: false,
                focus: None,
                active: true,
            });
        })
    });
    run(&view, cx, Command::FocusRight);
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t1")))
        );
        assert!(view.read(cx).live.activation_pending());
    });
}

#[gpui::test]
fn held_core_cycle_commits_only_its_final_selection(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2", "t3", "t4"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_core_pane(tab, &id("t1"), window, cx)
        })
    });
    run(&view, cx, Command::FocusRight);
    run(&view, cx, Command::FocusRight);
    run(&view, cx, Command::FocusRight);
    cx.update(|window, cx| {
        crate::bind_keys(cx);
        window.draw(cx).clear(cx);
    });
    cx.simulate_modifiers_change(Modifiers::control());
    for expected in ["t3", "t2"] {
        cx.simulate_keystrokes("ctrl-tab");
        cx.update(|window, cx| {
            assert_eq!(
                view.read(cx).core_command_pane(window, cx),
                Some((tab, id(expected)))
            );
        });
    }
    cx.simulate_modifiers_change(Modifiers::none());
    cx.simulate_modifiers_change(Modifiers::control());
    cx.simulate_keystrokes("ctrl-tab");
    cx.simulate_modifiers_change(Modifiers::none());
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t4")))
        );
    });
}

#[gpui::test]
fn rapid_core_click_is_observed_at_the_next_command(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2", "t3"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_core_pane(tab, &id("t1"), window, cx);
            view.observe_recent_focus(window, cx);
            let next = view.core_terminals.views[&(tab, id("t3"))]
                .read(cx)
                .focus_handle()
                .clone();
            window.focus(&next, cx);
        })
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t1")))
        );
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t3")))
        );
    });
}

#[gpui::test]
fn fallback_shell_and_a_tab_share_recent_history(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "shell"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let shell = view
                .core_terminals
                .views
                .remove(&(tab, id("shell")))
                .unwrap();
            view.core_terminals
                .tabs
                .get_mut(&tab)
                .unwrap()
                .panes
                .remove(&id("shell"));
            view.core_terminals.fallback = Fallback::Shown(shell.clone());
            view.live.snapshot = None;
            view.live.missing_installation = true;
            view.refresh_core_session(cx);
            assert!(Store::update(cx, |store| store.move_to_workspace(tab, WORKSPACE)));
            view.show_core_pane(tab, &id("t1"), window, cx);
            view.observe_recent_focus(window, cx);
            let group = view.active_group().unwrap();
            view.ensure_layout().unwrap().follow_terminal(group);
            window.focus(&shell.read(cx).focus_handle().clone(), cx);
        })
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t1")))
        );
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        let view = view.read(cx);
        assert_eq!(
            view.group_shown(view.active_group().unwrap(), cx),
            Shown::Terminal
        );
        assert!(
            view.core_fallback_view()
                .unwrap()
                .read(cx)
                .focus_handle()
                .is_focused(window)
        );
    });
}

#[gpui::test]
fn a_stale_core_destination_is_skipped(cx: &mut TestAppContext) {
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2", "t3"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_core_pane(tab, &id("t1"), window, cx)
        })
    });
    run(&view, cx, Command::FocusRight);
    run(&view, cx, Command::FocusRight);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.core_terminals.views[&(tab, id("t2"))]
                .clone()
                .update(cx, |view, cx| view.set_stale(cx))
        })
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t1")))
        );
    });
}

#[gpui::test]
fn core_composition_keeps_the_recent_command_on_its_pane(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler;
    let (view, cx, tab) = window_with_split(cx, &["t1", "t2"]);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.show_core_pane(tab, &id("t1"), window, cx)
        })
    });
    run(&view, cx, Command::FocusRight);
    cx.update(|window, cx| {
        let pane = view.read(cx).core_terminals.views[&(tab, id("t2"))].clone();
        pane.update(cx, |pane, cx| {
            pane.replace_and_mark_text_in_range(None, "か", None, window, cx)
        });
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t2")))
        );
    });
}
