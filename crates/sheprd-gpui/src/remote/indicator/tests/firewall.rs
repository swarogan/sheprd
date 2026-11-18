//! The title bar's firewall warning: a badge on the phone, and a line over
//! the list of phones, only while the approval lasts for this build alone.

use super::*;
use sheprd_remote_control::app::firewall::Approval;

fn open_menu(cx: &mut VisualTestContext) {
    let remote = cx.debug_bounds("titlebar-remote").unwrap();
    cx.simulate_click(remote.center(), Modifiers::none());
    draw(cx);
}

#[gpui::test]
fn a_per_build_approval_badges_the_phone_and_explains_in_the_menu(cx: &mut TestAppContext) {
    let (_view, cx) = window(cx, vec![phone("a", "iPhone", true)], 1);
    assert!(cx.debug_bounds("titlebar-remote-firewall").is_none());
    cx.update(|_, cx| Supervisor::set_approval_for_tests(cx, Approval::ThisBuild));
    draw(cx);
    assert!(cx.debug_bounds("titlebar-remote-firewall").is_some());
    assert!(
        cx.debug_bounds("titlebar-remote-live").is_none(),
        "the warning takes the badge's corner"
    );
    open_menu(cx);
    assert!(cx.debug_bounds("remote-menu-firewall").is_some());
}

#[gpui::test]
fn a_lasting_approval_shows_no_warning(cx: &mut TestAppContext) {
    let (_view, cx) = window(cx, vec![phone("a", "iPhone", true)], 1);
    assert!(cx.debug_bounds("titlebar-remote-live").is_some());
    open_menu(cx);
    assert!(cx.debug_bounds("titlebar-remote-firewall").is_none());
    assert!(cx.debug_bounds("remote-menu-firewall").is_none());
}
