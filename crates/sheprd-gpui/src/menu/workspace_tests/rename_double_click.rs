use super::*;
use gpui::{MouseButton, MouseDownEvent, MouseUpEvent, point, px};

/// Double-clicking the name in a space's Rename dialog selects all of it, so
/// one keystroke replaces the old name; a single click only places the caret.
#[gpui::test]
fn double_click_in_the_rename_field_selects_the_whole_name(cx: &mut gpui::TestAppContext) {
    let (view, cx) = cx.add_window_view(sidebar::layout_tests::fixture_window);
    cx.simulate_resize(gpui::size(px(800.), px(600.)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let snapshot = std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap());
            snapshot.workspaces = sidebar::layout_tests::snapshot(7).workspaces;
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.menu.reset();
            view.open_workspace_menu("w3", point(px(120.), px(140.)), window, cx);
            view.open_workspace_dialog(WorkspaceAction::Rename, window, cx);
        });
        window.draw(cx).clear(cx);
    });
    let name = view.read_with(cx, |view, _| view.menu.input.as_ref().unwrap().text.clone());
    assert!(!name.is_empty());
    let field = cx.debug_bounds("dialog-input").unwrap();
    // Past the name's end, so the caret lands after it.
    let position = point(field.right() - px(4.), field.center().y);
    let selection = |cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| {
            view.menu.input.as_ref().unwrap().selection.clone()
        })
    };

    cx.simulate_click(position, gpui::Modifiers::default());
    assert_eq!(selection(cx), name.len()..name.len());

    for click_count in 1..=2 {
        cx.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count,
            ..Default::default()
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    assert_eq!(selection(cx), 0..name.len());
    // The dialog is still open on the same space.
    view.read_with(cx, |view, _| {
        assert_eq!(target_id(view), Some("w3"));
        assert!(view.menu.page.is_some());
    });
}
