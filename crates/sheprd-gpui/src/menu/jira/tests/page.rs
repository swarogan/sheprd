//! The page in a window: every account page's header reaches it, an address,
//! email, or token that cannot be one is refused before any request, and a
//! finished save empties the token field only while it still holds the saved
//! token.
use super::*;
use secrecy::SecretString;
use sheprd_orchestrator::jira;

fn type_in(
    view: &Entity<SheprdWindow>,
    cx: &mut VisualTestContext,
    (address, email, token): (&str, &str, &str),
) {
    view.update(cx, |view, cx| {
        let jira = &view.menu.jira;
        let (Some(site), Some(mail), Some(secret)) =
            (jira.address.clone(), jira.email.clone(), jira.token.clone())
        else {
            panic!("the page makes its fields");
        };
        site.update(cx, |input, cx| input.set_text_selected(address, cx));
        mail.update(cx, |input, cx| input.set_text_selected(email, cx));
        secret.update(cx, |input, cx| input.set_text_selected(token, cx));
    });
}

fn message_and_token(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> (String, String) {
    view.read_with(cx, |view, cx| {
        (
            plain(view.menu.jira.message.clone().unwrap_or_default()),
            view.menu
                .jira
                .token
                .as_ref()
                .map(|input| input.read(cx).text().to_owned())
                .unwrap_or_default(),
        )
    })
}

#[gpui::test]
fn every_account_pages_header_opens_the_jira_page(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    cx.simulate_resize(size(px(1200.), px(900.)));
    // No window reads a Jira source: the tab is there regardless.
    view.update_in(cx, |view, window, cx| {
        assert!(view.open_menu(window, cx));
        view.menu.page = Some(Page::GitHub);
        cx.notify();
    });
    draw(cx);
    let pages = [
        ("accounts-tab-jira", Page::Jira),
        ("accounts-tab-gitlab", Page::GitLab),
        ("accounts-tab-jira", Page::Jira),
        ("accounts-tab-gitea", Page::Gitea),
        ("accounts-tab-jira", Page::Jira),
        ("accounts-tab-azure", Page::AzureDevOps),
        ("accounts-tab-jira", Page::Jira),
        ("accounts-tab-github", Page::GitHub),
    ];
    for (tab, shown) in pages {
        click(cx, tab);
        assert!(
            view.read_with(cx, |view, _| view.menu.page) == Some(shown),
            "{tab}"
        );
    }
    click(cx, "accounts-tab-jira");
    for field in ["jira-address-input", "jira-email-input", "jira-token-input"] {
        assert!(cx.debug_bounds(field).is_some(), "{field}");
    }
}

#[gpui::test]
fn every_tab_fits_in_the_header(cx: &mut TestAppContext) {
    let (_view, cx) = page(cx);
    let header = cx.debug_bounds("github-header-close").unwrap();
    let panel = cx.debug_bounds("jira-body").unwrap();
    for tab in [
        "accounts-tab-github",
        "accounts-tab-gitlab",
        "accounts-tab-gitea",
        "accounts-tab-azure",
        "accounts-tab-jira",
    ] {
        let bounds = cx.debug_bounds(tab).unwrap();
        assert!(
            bounds.right() <= header.left() && bounds.left() >= panel.left(),
            "{tab} at {bounds:?} runs into Close at {header:?} or out of the panel {panel:?}"
        );
    }
    assert!(header.right() <= panel.right(), "{header:?} in {panel:?}");
}

#[gpui::test]
fn an_address_email_or_token_that_cannot_be_one_is_refused_before_any_request(
    cx: &mut TestAppContext,
) {
    let (view, cx) = page(cx);
    for (typed, said) in [
        (
            ("", "me@example.com", "tok"),
            "Type the site's address first.",
        ),
        // Jira sites are https only.
        (
            ("http://acme.atlassian.net", "me@example.com", "tok"),
            "\"http://acme.atlassian.net\" is not an https address of a Jira Cloud site, such as https://acme.atlassian.net",
        ),
        (
            ("acme.atlassian.net", "", "tok"),
            "Type your Atlassian email first.",
        ),
        (
            ("acme.atlassian.net", "not an email", "tok"),
            "The Atlassian account email must be an address such as you@example.com",
        ),
        (
            ("acme.atlassian.net", "me@example.com", ""),
            "Paste an API token first.",
        ),
        // A headless window keeps no saved credential.
        (
            ("acme.atlassian.net", "me@example.com", "tok"),
            "This window does not keep Jira sign-ins.",
        ),
    ] {
        type_in(&view, cx, typed);
        click(cx, "jira-save");
        let (message, _) = message_and_token(&view, cx);
        assert_eq!(message, said, "{typed:?}");
        assert!(!view.read_with(cx, |view, _| view.menu.jira.working));
    }
}

#[gpui::test]
fn a_finished_save_keeps_a_token_typed_after_it(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    let typed = |token| ("acme.atlassian.net", "me@example.com", token);
    for (token, cleared) in [("tok-next", false), ("tok-saved", true)] {
        type_in(&view, cx, typed(token));
        view.update(cx, |view, cx| {
            let saved = Some(SecretString::from("tok-saved"));
            view.run_jira(cx, saved, || Ok("Saved.".to_owned()));
        });
        draw(cx);
        let (message, token_left) = message_and_token(&view, cx);
        assert_eq!(message, "Saved.");
        assert_eq!(token_left.is_empty(), cleared, "{token}");
    }
    // A failed check keeps the token, so a mistyped address costs nothing.
    type_in(&view, cx, typed("tok-refused"));
    view.update(cx, |view, cx| {
        let saved = Some(SecretString::from("tok-refused"));
        view.run_jira(cx, saved, || Err(jira::Error::Authentication));
    });
    draw(cx);
    let (message, token) = message_and_token(&view, cx);
    assert!(message.starts_with("Jira rejected the email"), "{message}");
    assert_eq!(token, "tok-refused");
}

#[gpui::test]
fn a_saved_site_offers_forget(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    view.update(cx, |view, _| {
        view.show_jira_sites(
            vec![
                (
                    "acme.atlassian.net".into(),
                    "https://acme.atlassian.net".into(),
                    "me@example.com".into(),
                ),
                (
                    "forge.example.com".into(),
                    "https://forge.example.com".into(),
                    "me@example.com".into(),
                ),
            ],
            || Lookup::Unknown,
        );
    });
    draw(cx);
    assert!(cx.debug_bounds("jira-site-0").is_some());
    assert!(cx.debug_bounds("jira-forget-0").is_some());
    assert!(cx.debug_bounds("jira-forget-1").is_some());
    // Forget in a headless window says it keeps no sign-ins.
    click(cx, "jira-forget-0");
    let message = view.read_with(cx, |view, _| view.menu.jira.message.clone());
    assert_eq!(
        message.map(plain).as_deref(),
        Some("This window does not keep Jira sign-ins.")
    );
}
