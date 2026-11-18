use super::*;

#[gpui::test]
fn source_switch_locates_each_saved_theme_without_changing_it(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture);
    view.update(cx, |view, cx| {
        view.config.theme = "Nord Light".into();
        view.shared =
            Some(herdr_settings::Settings::parse_text("[theme]\nname = 'terminal'\n").unwrap());
        view.themes.names = vec!["Dracula".into(), "Nord Light".into()];
        let app = Choice {
            scope: Scope::App,
            name: "Nord Light".into(),
        };
        let herdr = Choice {
            scope: Scope::Herdr,
            name: "terminal".into(),
        };
        view.themes.filter(Some(&app));
        let original_theme = view.theme.clone();

        view.set_theme_sources(false, true, cx);
        assert_eq!(view.themes.choice(), Some(herdr.clone()));
        assert!(view.theme_card_saved(&herdr));
        assert!(!view.theme_card_saved(&Choice {
            scope: Scope::Herdr,
            name: "catppuccin".into(),
        }));

        view.set_theme_sources(true, false, cx);
        assert_eq!(view.themes.choice(), Some(app.clone()));
        assert!(view.theme_card_saved(&app));
        assert!(!view.theme_card_saved(&Choice {
            scope: Scope::App,
            name: "Dracula".into(),
        }));

        view.set_theme_sources(true, true, cx);
        assert!(view.theme_card_saved(&app));
        assert!(view.theme_card_saved(&herdr));
        assert_eq!(view.theme, original_theme);
        assert_eq!(view.config.theme, "Nord Light");
        assert_eq!(view.shared.as_ref().unwrap().theme_name, "terminal");
        assert!(view.theme_intent.is_none());
    });
}

#[gpui::test]
fn filtered_out_current_theme_does_not_mark_fallback_as_saved(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture);
    view.update(cx, |view, cx| {
        view.config.theme = "Nord Light".into();
        view.themes.names = vec!["Dracula".into(), "Nord Light".into()];
        view.themes.query = "dracula".into();

        view.set_theme_sources(true, false, cx);
        let fallback = view.themes.choice().unwrap();
        assert_eq!(fallback.name, "Dracula");
        assert!(!view.theme_card_saved(&fallback));
        assert!(!view.theme_card_draft(&fallback));
        assert!(view.theme_intent.is_none());
        assert_eq!(view.config.theme, "Nord Light");
    });
}

#[gpui::test]
fn switching_sources_preserves_a_pending_herdr_theme(cx: &mut TestAppContext) {
    if !Scope::Herdr.editable() {
        return;
    }
    let (view, cx) = cx.add_window_view(fixture);
    view.update(cx, |view, cx| {
        view.config.theme = "Nord Light".into();
        view.shared =
            Some(herdr_settings::Settings::parse_text("[theme]\nname = 'terminal'\n").unwrap());
        view.themes.names = vec!["Nord Light".into()];
        view.themes.filter(None);
        let herdr_draft = Choice {
            scope: Scope::Herdr,
            name: "nord".into(),
        };
        let index = view
            .themes
            .filtered
            .iter()
            .position(|choice| choice == &herdr_draft)
            .unwrap();
        view.select_settings_theme(index, cx);
        assert!(view.theme_card_draft(&herdr_draft));

        view.set_theme_sources(true, false, cx);
        assert_eq!(view.themes.choice().unwrap().name, "Nord Light");
        view.set_theme_sources(false, true, cx);
        assert_eq!(view.themes.choice(), Some(herdr_draft.clone()));
        assert!(view.theme_card_draft(&herdr_draft));
        assert_eq!(view.shared.as_ref().unwrap().theme_name, "terminal");
        assert_eq!(view.theme_intent.as_ref().unwrap().choice, herdr_draft);
    });
}

#[gpui::test]
fn source_switch_locates_a_saved_herdr_alias(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(fixture);
    view.update(cx, |view, cx| {
        view.shared =
            Some(herdr_settings::Settings::parse_text("[theme]\nname = 'tokyonight'\n").unwrap());
        let herdr = Choice {
            scope: Scope::Herdr,
            name: "tokyo-night".into(),
        };

        view.set_theme_sources(false, true, cx);
        assert_eq!(view.themes.choice(), Some(herdr.clone()));
        assert!(view.theme_card_saved(&herdr));
        assert!(view.theme_intent.is_none());
    });
}
