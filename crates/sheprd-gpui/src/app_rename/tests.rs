#![allow(clippy::unwrap_used)]
use super::*;
use crate::menu::Page;

/// A `Herdr.app` in a fresh folder, as a standalone install or one `cask` owns.
fn legacy(cask: Option<&'static str>) -> (tempfile::TempDir, Legacy) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Herdr.app");
    std::fs::create_dir(&path).unwrap();
    let bundle = Legacy::at(&path, |_| cask).unwrap();
    (root, bundle)
}

fn offered(
    bundle: Legacy,
    cx: &mut gpui::TestAppContext,
) -> (
    gpui::Entity<crate::window::SheprdWindow>,
    &mut gpui::VisualTestContext,
) {
    use crate::sidebar::layout_tests::fixture_window;
    cx.update(|cx| AppRename::unasked(bundle, None).install(cx));
    let (view, cx) = cx.add_window_view(fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.active = true;
            view.offer_app_rename(window, cx);
            assert_eq!(view.menu.page, Some(Page::AppRename));
        });
    });
    cx.run_until_parked();
    (view, cx)
}

#[test]
fn choices_are_saved_as_json() {
    let text = serde_json::to_string(&Saved {
        choice: Choice::Declined,
    })
    .unwrap();
    assert_eq!(text, r#"{"choice":"declined"}"#);
    for (json, choice) in [
        (r#"{"choice":"renamed"}"#, Choice::Renamed),
        (r#"{"choice":"copied"}"#, Choice::Copied),
    ] {
        let saved: Saved = serde_json::from_str(json).unwrap();
        assert_eq!(saved.choice, choice);
    }
}

#[gpui::test]
fn the_offer_is_made_once_and_closing_it_means_not_now(cx: &mut gpui::TestAppContext) {
    let (_root, bundle) = legacy(None);
    let (view, cx) = offered(bundle, cx);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            // Another window, or a later tick, does not ask again.
            view.dismiss_menu(window, cx);
            view.offer_app_rename(window, cx);
            assert_eq!(view.menu.page, None);
        });
        assert!(AppRename::bundle(cx).is_none());
    });
}

#[gpui::test]
fn a_standalone_app_is_offered_the_move(cx: &mut gpui::TestAppContext) {
    let (root, bundle) = legacy(None);
    let (_view, cx) = offered(bundle, cx);
    assert!(cx.debug_bounds("app-rename-offer").is_some());
    assert!(cx.debug_bounds("app-rename-confirm").is_some());
    assert!(cx.debug_bounds("app-rename-commands").is_none());
    let renamed = root.path().join("Sheprd.app");
    cx.update(|_, cx| {
        let Some(Legacy::Standalone(bundle)) = AppRename::bundle(cx) else {
            panic!("not standalone");
        };
        assert_eq!(bundle.renamed(), renamed);
    });
}

#[gpui::test]
fn a_homebrew_app_is_shown_and_copies_the_reinstall_commands(cx: &mut gpui::TestAppContext) {
    let (_root, bundle) = legacy(Some("herdr-gpui"));
    let (view, cx) = offered(bundle, cx);
    assert!(cx.debug_bounds("app-rename-commands").is_some());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.confirm_app_rename(window, cx);
            assert_eq!(view.menu.page, None);
        });
        assert!(AppRename::bundle(cx).is_none());
    });
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        copied.as_deref(),
        Some("brew uninstall --cask herdr-gpui\nbrew install sheprd-labs/tap/sheprd")
    );
}

#[test]
fn a_development_build_never_asks() {
    if !crate::RELEASE_BUILD {
        let rename = AppRename::load();
        assert!(!rename.ask && rename.bundle.is_none() && rename.path.is_none());
    }
}

#[gpui::test]
fn closing_the_window_while_asked_saves_not_now(cx: &mut gpui::TestAppContext) {
    use crate::sidebar::layout_tests::fixture_window;
    let (root, bundle) = legacy(None);
    let saved = root.path().join("app-rename.json");
    cx.update(|cx| AppRename::unasked(bundle, Some(saved.clone())).install(cx));
    let (view, vcx) = cx.add_window_view(fixture_window);
    vcx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.active = true;
            view.offer_app_rename(window, cx);
            assert_eq!(view.menu.page, Some(Page::AppRename));
        });
    });
    vcx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    cx.update(|cx| assert!(AppRename::bundle(cx).is_none()));
    let saved: Saved = serde_json::from_slice(&std::fs::read(&saved).unwrap()).unwrap();
    assert_eq!(saved.choice, Choice::Declined);
}
