//! The page from the keyboard: Tab walks its controls and fields in the order
//! they show, and Enter or Space uses the selected control.
use super::*;
use crate::menu::jira::keys::Action;

fn sites(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, hosts: &[&str]) {
    view.update(cx, |view, cx| {
        let hosts = hosts
            .iter()
            .map(|host| {
                (
                    (*host).to_owned(),
                    format!("https://{host}"),
                    "me@example.com".to_owned(),
                )
            })
            .collect();
        view.show_jira_sites(hosts, || Lookup::Unknown);
        cx.notify();
    });
    draw(cx);
}

fn selected(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> Option<Action> {
    view.read_with(cx, |view, _| view.menu.jira.selected.clone())
}

fn message(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, _| {
        plain(view.menu.jira.message.clone().unwrap_or_default())
    })
}

#[gpui::test]
fn tab_walks_every_control_from_the_token_field(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    sites(&view, cx, &["acme.atlassian.net", "forge.example.com"]);
    let actions = view.read_with(cx, |view, _| view.menu.jira.actions());
    assert_eq!(
        actions,
        [
            Action::GitHub,
            Action::GitLab,
            Action::Gitea,
            Action::AzureDevOps,
            Action::Forget("acme.atlassian.net".into()),
            Action::Forget("forge.example.com".into()),
            Action::Address,
            Action::Email,
            Action::Token,
            Action::CreateToken,
            Action::Save,
            Action::Close,
        ],
        "every saved site offers Forget, and the other tabs come first"
    );
    let start = actions.iter().position(|action| *action == Action::Token);
    for step in 1..=actions.len() {
        cx.simulate_keystrokes("tab");
        let at = (start.unwrap() + step) % actions.len();
        assert_eq!(selected(&view, cx).as_ref(), Some(&actions[at]));
    }
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(selected(&view, cx), Some(Action::Email));
}

#[gpui::test]
fn a_field_takes_focus_and_keeps_its_own_space(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    cx.simulate_keystrokes("shift-tab shift-tab");
    let focused = cx.update(|window, cx| {
        view.read(cx)
            .menu
            .jira
            .address
            .as_ref()
            .is_some_and(|input| input.read(cx).focus.is_focused(window))
    });
    assert!(
        focused,
        "Shift-Tab from the token field reaches the address, past the email"
    );
    cx.simulate_input("two words");
    let typed = view.read_with(cx, |view, cx| {
        SheprdWindow::typed_jira(&view.menu.jira.address, cx)
    });
    assert_eq!(typed, "two words");
    assert!(selected(&view, cx) == Some(Action::Address));
}

#[gpui::test]
fn enter_uses_the_selected_control(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    sites(&view, cx, &["acme.atlassian.net"]);
    // Back from the token field: the email, the address, then Forget.
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab");
    assert_eq!(
        selected(&view, cx),
        Some(Action::Forget("acme.atlassian.net".into()))
    );
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(
        message(&view, cx),
        "This window does not keep Jira sign-ins."
    );
    cx.simulate_keystrokes("shift-tab enter");
    draw(cx);
    assert!(view.read_with(cx, |view, _| view.menu.page) == Some(Page::AzureDevOps));
}

#[gpui::test]
fn enter_in_a_field_saves_and_escape_closes(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(message(&view, cx), "Type the site's address first.");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(view.read_with(cx, |view, _| view.menu.page) != Some(Page::Jira));
}

#[gpui::test]
fn a_forgotten_sites_control_is_not_used(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    sites(&view, cx, &["acme.atlassian.net"]);
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab");
    sites(&view, cx, &[]);
    cx.simulate_keystrokes("space");
    draw(cx);
    assert_eq!(message(&view, cx), "");
}

#[gpui::test]
fn the_selection_scrolls_into_view_both_ways(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    let hosts: Vec<String> = (0..30)
        .map(|index| format!("forge{index:02}.test"))
        .collect();
    sites(
        &view,
        cx,
        &hosts.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let shown = |cx: &mut VisualTestContext, selector: String| {
        let selector: &'static str = selector.leak();
        draw(cx);
        let body = cx.debug_bounds("jira-body").unwrap();
        let control = cx.debug_bounds(selector).unwrap();
        assert!(
            control.top() >= body.top() && control.bottom() <= body.bottom(),
            "{selector} at {control:?} is outside {body:?}"
        );
    };
    cx.simulate_keystrokes("shift-tab");
    shown(cx, "jira-email-input".into());
    cx.simulate_keystrokes("shift-tab");
    shown(cx, "jira-address-input".into());
    for index in (0..hosts.len()).rev() {
        cx.simulate_keystrokes("shift-tab");
        shown(cx, format!("jira-forget-{index}"));
    }
    for index in 1..hosts.len() {
        cx.simulate_keystrokes("tab");
        shown(cx, format!("jira-forget-{index}"));
    }
}

#[gpui::test]
fn each_stop_names_its_child_of_the_body(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    sites(&view, cx, &["acme.atlassian.net", "forge.example.com"]);
    view.update(cx, |view, _| {
        let jira = &mut view.menu.jira;
        // The help line, then two cards.
        assert_eq!(
            jira.body_child(&Action::Forget("acme.atlassian.net".into())),
            Some(1)
        );
        assert_eq!(
            jira.body_child(&Action::Forget("forge.example.com".into())),
            Some(2)
        );
        assert_eq!(jira.body_child(&Action::Address), Some(3));
        assert_eq!(jira.body_child(&Action::Email), Some(4));
        assert_eq!(jira.body_child(&Action::Token), Some(5));
        assert_eq!(jira.body_child(&Action::CreateToken), Some(6));
        assert_eq!(jira.body_child(&Action::Save), None);
        jira.problem = Some("unreadable".into());
        assert_eq!(
            jira.body_child(&Action::Forget("acme.atlassian.net".into())),
            Some(2)
        );
    });
}

#[gpui::test]
fn the_token_link_opens_where_atlassian_creates_one(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    // From the token field, the link is next.
    cx.simulate_keystrokes("tab");
    assert_eq!(selected(&view, cx), Some(Action::CreateToken));
    assert!(cx.opened_url().is_none());
    cx.simulate_keystrokes("enter");
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://id.atlassian.com/manage-profile/security/api-tokens")
    );
}
