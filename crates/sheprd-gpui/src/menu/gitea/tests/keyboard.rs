//! The page from the keyboard: Tab walks its controls and fields in the order
//! they show, and Enter or Space uses the selected control.
use super::*;
use crate::menu::gitea::keys::Action;

fn instances(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, hosts: &[&str]) {
    view.update(cx, |view, cx| {
        let hosts = hosts
            .iter()
            .map(|host| ((*host).to_owned(), format!("https://{host}")))
            .collect();
        view.show_gitea_instances(hosts, || Lookup::Unknown);
        cx.notify();
    });
    draw(cx);
}

fn selected(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> Option<Action> {
    view.read_with(cx, |view, _| view.menu.gitea.selected.clone())
}

fn message(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, _| {
        plain(view.menu.gitea.message.clone().unwrap_or_default())
    })
}

#[gpui::test]
fn tab_walks_every_control_from_the_token_field(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    instances(&view, cx, &["gitea.arpa", "forge.example.com"]);
    let actions = view.read_with(cx, |view, _| view.menu.gitea.actions());
    assert_eq!(
        actions,
        [
            Action::GitHub,
            Action::GitLab,
            Action::AzureDevOps,
            Action::Jira,
            Action::Forget("gitea.arpa".into()),
            Action::Forget("forge.example.com".into()),
            Action::Address,
            Action::Token,
            Action::CreateToken,
            Action::Save,
            Action::Close,
        ],
        "every saved instance offers Forget, and the other tabs come first"
    );
    let start = actions.iter().position(|action| *action == Action::Token);
    for step in 1..=actions.len() {
        cx.simulate_keystrokes("tab");
        let at = (start.unwrap() + step) % actions.len();
        assert_eq!(selected(&view, cx).as_ref(), Some(&actions[at]));
    }
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(selected(&view, cx), Some(Action::Address));
}

#[gpui::test]
fn a_field_takes_focus_and_keeps_its_own_space(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    cx.simulate_keystrokes("shift-tab");
    let focused = cx.update(|window, cx| {
        view.read(cx)
            .menu
            .gitea
            .address
            .as_ref()
            .is_some_and(|input| input.read(cx).focus.is_focused(window))
    });
    assert!(
        focused,
        "Shift-Tab from the token field reaches its neighbor"
    );
    cx.simulate_input("two words");
    let typed = view.read_with(cx, |view, cx| view.typed_gitea_address(cx));
    assert_eq!(typed, "two words");
    assert!(selected(&view, cx) == Some(Action::Address));
}

#[gpui::test]
fn enter_uses_the_selected_control(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    instances(&view, cx, &["gitea.arpa"]);
    // Back from the token field: the address, then Forget.
    cx.simulate_keystrokes("shift-tab shift-tab");
    assert_eq!(
        selected(&view, cx),
        Some(Action::Forget("gitea.arpa".into()))
    );
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(
        message(&view, cx),
        "This window does not keep Gitea tokens."
    );
    cx.simulate_keystrokes("shift-tab enter");
    draw(cx);
    assert!(view.read_with(cx, |view, _| view.menu.page) == Some(Page::Jira));
}

#[gpui::test]
fn enter_in_a_field_saves_and_escape_closes(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(message(&view, cx), "Type the instance's address first.");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(view.read_with(cx, |view, _| view.menu.page) != Some(Page::Gitea));
}

#[gpui::test]
fn a_forgotten_instances_control_is_not_used(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    instances(&view, cx, &["gitea.arpa"]);
    cx.simulate_keystrokes("shift-tab shift-tab");
    instances(&view, cx, &[]);
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
    instances(
        &view,
        cx,
        &hosts.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let shown = |cx: &mut VisualTestContext, selector: String| {
        let selector: &'static str = selector.leak();
        draw(cx);
        let body = cx.debug_bounds("gitea-body").unwrap();
        let control = cx.debug_bounds(selector).unwrap();
        assert!(
            control.top() >= body.top() && control.bottom() <= body.bottom(),
            "{selector} at {control:?} is outside {body:?}"
        );
    };
    cx.simulate_keystrokes("shift-tab");
    shown(cx, "gitea-address-input".into());
    for index in (0..hosts.len()).rev() {
        cx.simulate_keystrokes("shift-tab");
        shown(cx, format!("gitea-forget-{index}"));
    }
    for index in 1..hosts.len() {
        cx.simulate_keystrokes("tab");
        shown(cx, format!("gitea-forget-{index}"));
    }
}

#[gpui::test]
fn each_stop_names_its_child_of_the_body(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    instances(&view, cx, &["gitea.arpa", "forge.example.com"]);
    view.update(cx, |view, _| {
        let gitea = &mut view.menu.gitea;
        // The help line, then two cards.
        assert_eq!(
            gitea.body_child(&Action::Forget("gitea.arpa".into())),
            Some(1)
        );
        assert_eq!(
            gitea.body_child(&Action::Forget("forge.example.com".into())),
            Some(2)
        );
        assert_eq!(gitea.body_child(&Action::Address), Some(3));
        assert_eq!(gitea.body_child(&Action::Token), Some(4));
        assert_eq!(gitea.body_child(&Action::Save), None);
        gitea.problem = Some("unreadable".into());
        assert_eq!(
            gitea.body_child(&Action::Forget("gitea.arpa".into())),
            Some(2)
        );
    });
}

#[gpui::test]
fn tab_from_the_token_field_reaches_the_link_before_save(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    cx.simulate_keystrokes("tab");
    assert_eq!(selected(&view, cx), Some(Action::CreateToken));
    cx.simulate_keystrokes("tab");
    assert_eq!(selected(&view, cx), Some(Action::Save));
}

#[gpui::test]
fn enter_on_the_link_without_an_address_asks_for_it(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    cx.simulate_keystrokes("tab enter");
    draw(cx);
    assert_eq!(message(&view, cx), "Type the instance's address first.");
}
