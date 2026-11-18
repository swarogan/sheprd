use super::*;

#[gpui::test]
fn a_supported_source_skips_an_unsupported_remembered_destination(cx: &mut TestAppContext) {
    let (mut local, mut remote) = (fenced(), fenced());
    let (view, cx) = two_machines(cx, &local, &remote);
    view.update(cx, |view, _| {
        view.endpoints[1].live.supports_surface = false
    });
    tap(cx, Modifiers::control(), "ctrl-tab");
    assert_eq!(switched(&view, cx).0, LOCAL);
    assert_eq!(focus_requests(&mut local), ["c"]);
    assert!(focus_requests(&mut remote).is_empty());
}
