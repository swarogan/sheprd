use super::*;

#[gpui::test]
fn a_kept_workspace_closes_without_waiting_on_a_herdr_1_git_check(cx: &mut TestAppContext) {
    let (view, cx, id) = window(cx, PLAIN, None);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_workspace_dialog_for(
                &id,
                WorkspaceAction::Close,
                gpui::Point::default(),
                window,
                cx,
            );
            assert!(view.menu.close_check.is_none());
        });
        window.draw(cx).clear(cx);
    });
    assert!(cx.debug_bounds("close-git-status").is_none());
    let submit = cx.debug_bounds("dialog-submit").unwrap();
    cx.simulate_click(submit.center(), gpui::Modifiers::none());
    view.read_with(cx, |view, _| {
        assert!(view.menu.page.is_none());
        assert!(!view.is_kept_workspace(&id));
    });
}
