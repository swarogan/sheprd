//! Deploying a relay on DigitalOcean, beside adding one. With no relay
//! listed, two cards offer both ways in; once one is, a row under Add
//! keeps the way to deploy another. Both open `sheprd.org/relay`, which
//! creates the droplets from the browser, so no DigitalOcean token ever
//! reaches the app.

use super::{SettingsWindow, ui};
use crate::{
    config::{Theme, corners},
    direction::Logical,
    i18n::fl,
};
use gpui::{prelude::*, *};
use sheprd_relay::directory::DEPLOY_URL;

/// The button that opens the deploy page: filled in the empty state, where
/// it is the way to start, plain beside a list that already has a relay.
fn deploy_button(theme: &Theme, filled: bool) -> Stateful<Div> {
    const ID: &str = "remote-relay-deploy";
    let label = fl!("relay-deploy-button");
    let (button, ink) = if filled {
        // ui::button's shape, filled: GPUI takes one hover style per
        // element, so this one is built whole rather than restyled.
        let fill = theme.primary();
        let ink = theme.text_on(fill);
        let button = div()
            .id(ID)
            .debug_selector(|| ID.into())
            .role(Role::Button)
            .flex_none()
            .whitespace_nowrap()
            .px(px(10.))
            .py(px(3.))
            .rounded(px(corners::CONTROL))
            .border_1()
            .border_color(rgb(fill))
            .bg(rgb(fill))
            .text_color(rgb(ink))
            .cursor_pointer()
            .hover(|style| style.opacity(0.85))
            .child(label);
        (button, ink)
    } else {
        (ui::button(theme, ID, label, true), theme.subtext())
    };
    button
        .aria_label(fl!("relay-deploy-label"))
        .row()
        .items_center()
        .gap(px(5.))
        .child(
            svg()
                .flex_none()
                .size(px(11.))
                .path("icons/external.svg")
                .text_color(rgb(ink)),
        )
        .on_click(|_, _, cx| cx.open_url(DEPLOY_URL))
}

fn card(theme: &Theme, id: &'static str, title: String, note: String) -> Div {
    div()
        .debug_selector(move || id.into())
        .flex_1()
        .min_w(px(220.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(12.))
        .rounded(px(corners::PANEL))
        .border_1()
        .border_color(rgb(theme.active))
        .bg(rgb(theme.surface))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(11.))
                .line_height(px(15.))
                .text_color(rgb(theme.subtext()))
                .child(note),
        )
}

impl SettingsWindow {
    /// The empty state: deploy a relay, or add the one you run. `field` is
    /// the address field and `add` its button, as the Add row shows them.
    pub(super) fn relay_start(&self, field: Div, add: Stateful<Div>) -> Div {
        let theme = &self.theme;
        let existing_note = if self.remote.relay_invalid {
            fl!(
                "settings-remote-relay-invalid",
                example = super::PLACEHOLDER
            )
        } else {
            fl!("relay-deploy-existing-note")
        };
        div()
            .row()
            .flex_wrap()
            .gap(px(10.))
            .py(px(10.))
            .child(
                card(
                    theme,
                    "remote-relay-deploy-card",
                    fl!("relay-deploy-title"),
                    fl!("relay-deploy-note"),
                )
                .child(div().row().child(deploy_button(theme, true))),
            )
            .child(
                card(
                    theme,
                    "remote-relay-add",
                    fl!("relay-deploy-existing"),
                    existing_note,
                )
                .child(
                    div()
                        .row()
                        .items_center()
                        .gap(px(8.))
                        .child(field.flex_1().min_w_0())
                        .child(add),
                ),
            )
    }

    /// Under the Add row, once a relay is listed.
    pub(super) fn relay_deploy_row(&self) -> impl IntoElement {
        let theme = &self.theme;
        ui::row(theme, "remote-relay-deploy-row", fl!("relay-deploy-row"))
            .description(fl!("relay-deploy-row-note"))
            .control(deploy_button(theme, false))
    }
}
