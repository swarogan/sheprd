use super::*;
use crate::{config::Theme, herdr_settings::Settings};
use sheprd_client::protocol::AgentStatus;

/// The overview and the device picker's Agent Activity card share this look.
#[gpui::test]
fn terminal_theme_uses_app_colors_for_device_activity(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |view, cx| {
        view.theme = Theme::builtin("Catppuccin Macchiato").unwrap();
        view.settings.shared = Some(Settings::parse_text("[theme]\nname='terminal'\n").unwrap());
        let look = view.overview_look(cx);
        for (status, color) in [
            (AgentStatus::Working, view.theme.palette[3]),
            (AgentStatus::Blocked, view.theme.palette[1]),
        ] {
            assert_eq!(look.status(status), view.theme.ink(color));
        }

        let shared =
            Settings::parse_text("[theme]\nname='nord'\n[theme.custom]\nyellow='#123456'\n")
                .unwrap();
        let light = matches!(
            cx.window_appearance(),
            gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight
        );
        let expected = view
            .theme
            .ink(shared.status_color(AgentStatus::Working, light));
        view.settings.shared = Some(shared);
        assert_eq!(
            view.overview_look(cx).status(AgentStatus::Working),
            expected
        );
    });
}
