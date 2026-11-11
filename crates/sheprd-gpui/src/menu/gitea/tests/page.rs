//! The page in a window: every account page's header reaches it, an address
//! or token that cannot be one is refused before any request, an unencrypted
//! address is noted, and a finished save empties the token field only while
//! it still holds the saved token.
use super::*;
use secrecy::SecretString;
use sheprd_orchestrator::gitea;

fn type_in(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext, address: &str, token: &str) {
    view.update(cx, |view, cx| {
        let gitea = &view.menu.gitea;
        let (Some(field), Some(secret)) = (gitea.address.clone(), gitea.token.clone()) else {
            panic!("the page makes both fields");
        };
        field.update(cx, |input, cx| input.set_text_selected(address, cx));
        secret.update(cx, |input, cx| input.set_text_selected(token, cx));
    });
}

fn message_and_token(view: &Entity<SheprdWindow>, cx: &mut VisualTestContext) -> (String, String) {
    view.read_with(cx, |view, cx| {
        (
            plain(view.menu.gitea.message.clone().unwrap_or_default()),
            view.menu
                .gitea
                .token
                .as_ref()
                .map(|input| input.read(cx).text().to_owned())
                .unwrap_or_default(),
        )
    })
}

#[gpui::test]
fn every_account_pages_header_opens_the_gitea_page(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    cx.simulate_resize(size(px(1200.), px(900.)));
    // No window reads a Gitea source: the tab is there regardless.
    view.update_in(cx, |view, window, cx| {
        assert!(view.open_menu(window, cx));
        view.menu.page = Some(Page::GitHub);
        cx.notify();
    });
    draw(cx);
    let pages = [
        ("accounts-tab-gitea", Page::Gitea),
        ("accounts-tab-gitlab", Page::GitLab),
        ("accounts-tab-gitea", Page::Gitea),
        ("accounts-tab-azure", Page::AzureDevOps),
        ("accounts-tab-jira", Page::Jira),
        ("accounts-tab-gitea", Page::Gitea),
        ("accounts-tab-github", Page::GitHub),
    ];
    for (tab, shown) in pages {
        click(cx, tab);
        assert!(
            view.read_with(cx, |view, _| view.menu.page) == Some(shown),
            "{tab}"
        );
    }
    click(cx, "accounts-tab-gitea");
    assert!(cx.debug_bounds("gitea-address-input").is_some());
    assert!(cx.debug_bounds("gitea-token-input").is_some());
}

#[gpui::test]
fn every_tab_fits_in_the_header(cx: &mut TestAppContext) {
    let (_view, cx) = page(cx);
    let header = cx.debug_bounds("github-header-close").unwrap();
    let panel = cx.debug_bounds("gitea-body").unwrap();
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
fn an_address_or_token_that_cannot_be_one_is_refused_before_any_request(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    for (address, token, said) in [
        ("", "tok", "Type the instance's address first."),
        (
            "ftp://gitea.arpa",
            "tok",
            "\"ftp://gitea.arpa\" is not an address of a Gitea instance, such as http://gitea.example.com",
        ),
        ("gitea.arpa", "", "Paste a token first."),
        // A headless window keeps no saved credential.
        (
            "gitea.arpa",
            "tok",
            "This window does not keep Gitea tokens.",
        ),
    ] {
        type_in(&view, cx, address, token);
        click(cx, "gitea-save");
        let (message, _) = message_and_token(&view, cx);
        assert_eq!(message, said, "{address:?} {token:?}");
        assert!(!view.read_with(cx, |view, _| view.menu.gitea.working));
    }
}

#[gpui::test]
fn an_http_address_says_the_token_travels_unencrypted(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    for (address, noted) in [
        ("", false),
        ("gitea.example.com", false),
        ("https://gitea.example.com", false),
        ("http://192.168.1.5:3000", true),
    ] {
        type_in(&view, cx, address, "");
        draw(cx);
        assert_eq!(
            cx.debug_bounds("gitea-plain-http").is_some(),
            noted,
            "{address:?}"
        );
    }
}

#[gpui::test]
fn a_finished_save_keeps_a_token_typed_after_it(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    for (typed, cleared) in [("tok-next", false), ("tok-saved", true)] {
        type_in(&view, cx, "gitea.arpa", typed);
        view.update(cx, |view, cx| {
            let saved = Some(SecretString::from("tok-saved"));
            view.run_gitea(cx, saved, || Ok("Saved.".to_owned()));
        });
        draw(cx);
        let (message, token) = message_and_token(&view, cx);
        assert_eq!(message, "Saved.");
        assert_eq!(token.is_empty(), cleared, "{typed}");
    }
    // A failed check keeps the token, so a mistyped address costs nothing.
    type_in(&view, cx, "gitea.arpa", "tok-refused");
    view.update(cx, |view, cx| {
        let saved = Some(SecretString::from("tok-refused"));
        view.run_gitea(cx, saved, || Err(gitea::Error::Authentication));
    });
    draw(cx);
    let (message, token) = message_and_token(&view, cx);
    assert!(message.starts_with("Gitea rejected the token"), "{message}");
    assert_eq!(token, "tok-refused");
}

#[gpui::test]
fn a_saved_instance_offers_forget(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    view.update(cx, |view, _| {
        view.show_gitea_instances(
            vec![
                (
                    "forge.example.com".into(),
                    "https://forge.example.com".into(),
                ),
                ("gitea.arpa".into(), "http://gitea.arpa:3000".into()),
            ],
            || Lookup::Unknown,
        );
    });
    draw(cx);
    assert!(cx.debug_bounds("gitea-instance-0").is_some());
    assert!(cx.debug_bounds("gitea-forget-0").is_some());
    assert!(cx.debug_bounds("gitea-forget-1").is_some());
    // Forget in a headless window says it keeps no tokens.
    click(cx, "gitea-forget-0");
    let message = view.read_with(cx, |view, _| view.menu.gitea.message.clone());
    assert_eq!(
        message.map(plain).as_deref(),
        Some("This window does not keep Gitea tokens.")
    );
}

#[test]
fn the_token_page_is_under_the_instances_base_url() {
    use crate::menu::gitea::token_page;
    for (base, page) in [
        (
            "https://gitea.example.com",
            "https://gitea.example.com/user/settings/applications",
        ),
        (
            "https://gitea.example.com/",
            "https://gitea.example.com/user/settings/applications",
        ),
        (
            "http://192.168.1.5:3000",
            "http://192.168.1.5:3000/user/settings/applications",
        ),
        (
            "https://host/gitea",
            "https://host/gitea/user/settings/applications",
        ),
        (
            "https://host/gitea/",
            "https://host/gitea/user/settings/applications",
        ),
    ] {
        assert_eq!(token_page(base), page, "{base}");
    }
}

#[gpui::test]
fn the_page_links_to_creating_a_token(cx: &mut TestAppContext) {
    let (_view, cx) = page(cx);
    assert!(cx.debug_bounds("gitea-create-token").is_some());
}

#[gpui::test]
fn the_link_without_a_usable_address_opens_nothing_and_says_why(cx: &mut TestAppContext) {
    let (view, cx) = page(cx);
    for (address, said) in [
        ("", "Type the instance's address first."),
        (
            "ftp://gitea.arpa",
            "\"ftp://gitea.arpa\" is not an address of a Gitea instance, such as http://gitea.example.com",
        ),
    ] {
        type_in(&view, cx, address, "");
        click(cx, "gitea-create-token");
        let (message, _) = message_and_token(&view, cx);
        assert_eq!(message, said, "{address:?}");
    }
}
