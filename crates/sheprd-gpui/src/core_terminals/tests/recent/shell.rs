use super::*;
use crate::{
    SheprdWindow,
    browser::{Location, TabId},
    core_terminals::panes::Panes,
};
use gpui::{Entity, VisualTestContext};
use std::path::Path;

fn shell(cx: &mut TestAppContext) -> (Entity<SheprdWindow>, &mut VisualTestContext, TabId) {
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
            let group = view.ensure_layout().unwrap().active();
            view.set_primary_group(group);
            window.focus(&shell.read(cx).focus_handle().clone(), cx);
            view.observe_recent_focus(window, cx);
        })
    });
    (view, cx, tab)
}

#[gpui::test]
fn explicit_shell_selection_survives_the_kept_workspace_reveal(cx: &mut TestAppContext) {
    let (view, cx, old) = shell(cx);
    let tab = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let workspace = view
                .open_kept_workspace(
                    None,
                    Path::new("/src/other"),
                    Location::core_terminal(Panes::single(id("t1")), false),
                    window,
                    cx,
                )
                .unwrap();
            let scope = crate::browser::scope(&view.endpoints[0]);
            let tab = cx
                .global::<Store>()
                .in_workspace(&scope, &workspace)
                .next()
                .unwrap()
                .id;
            let pane = view.core_terminals.views.remove(&(old, id("t1"))).unwrap();
            view.core_terminals.views.insert((tab, id("t1")), pane);
            view.show_core_pane(tab, &id("t1"), window, cx);
            view.observe_recent_focus(window, cx);
            tab
        })
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.reveal_kept_workspace(window, cx);
            assert_eq!(view.browser_key().unwrap().1, WORKSPACE);
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
fn a_shell_destination_activates_its_primary_group(cx: &mut TestAppContext) {
    let (view, cx, tab) = shell(cx);
    let primary = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let primary = view.primary_group().unwrap();
            view.split_group(primary, window, cx);
            let secondary = view.active_group().unwrap();
            assert_ne!(secondary, primary);
            view.show_browser_tab_in(Some(secondary), tab, window, cx);
            view.observe_recent_focus(window, cx);
            primary
        })
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        let view = view.read(cx);
        assert_eq!(view.active_group(), Some(primary));
        assert_eq!(view.group_shown(primary, cx), Shown::Terminal);
        assert!(
            view.core_fallback_view()
                .unwrap()
                .read(cx)
                .focus_handle()
                .is_focused(window)
        );
    });
    run(&view, cx, Command::RecentPane);
    cx.update(|window, cx| {
        assert_eq!(
            view.read(cx).core_command_pane(window, cx),
            Some((tab, id("t1")))
        );
    });
}
