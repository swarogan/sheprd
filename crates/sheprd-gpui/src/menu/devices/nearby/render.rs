//! Drawing Nearby Sheprd: the computers found, then the code both screens
//! show, then the wait for the other computer's answer.

use super::{Found, NearbySetup, Step};
use crate::SheprdWindow;
use crate::fonts::StyledFont;
use crate::i18n::fl;
use crate::remote::nearby::Radio;
use gpui::{prelude::*, *};

impl SheprdWindow {
    pub(in crate::menu) fn render_add_nearby(
        &self,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let frame = |pane: Div, cx: &mut Context<Self>| {
            self.add_device_frame(
                "nearby-setup-dialog",
                "nearby-setup-close",
                compact,
                pane,
                cx,
            )
        };
        let Some(setup) = &self.menu.nearby_setup else {
            return frame(div(), cx);
        };
        let theme = &self.theme;
        let mut body = div()
            .id("nearby-setup-body")
            .debug_selector(|| "nearby-setup-body".into())
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex_none()
                    .text_color(rgb(theme.subtext()))
                    .child(fl!("devices-nearby-intro")),
            );
        body = match &setup.step {
            Step::Looking => body.child(self.nearby_list(setup, cx)),
            Step::Connecting(found) | Step::Exchanging(found) => body.child(
                div()
                    .debug_selector(|| "nearby-setup-status".into())
                    .text_color(rgb(theme.muted))
                    .child(fl!(
                        "devices-nearby-connecting",
                        computer = found.name.as_str()
                    )),
            ),
            Step::Code {
                computer,
                code,
                claimed,
            } => body.child(self.nearby_code(computer, code, *claimed)),
            Step::Saving(name) => body.child(
                div()
                    .debug_selector(|| "nearby-setup-status".into())
                    .text_color(rgb(theme.muted))
                    .child(fl!("devices-nearby-saving", computer = name.as_str())),
            ),
        };
        if let Some(error) = &setup.error {
            body = body.child(
                div()
                    .debug_selector(|| "nearby-setup-error".into())
                    .flex_none()
                    .text_color(crate::menu::danger(theme))
                    .child(error.clone()),
            );
        }
        let busy = setup.step != Step::Looking && !matches!(setup.step, Step::Saving(_));
        let footer = div()
            .flex_none()
            .p(px(16.))
            .border_t_1()
            .border_color(rgb(theme.active))
            .flex()
            .justify_end()
            .when(busy, |footer| {
                footer.child(
                    div()
                        .id("nearby-setup-cancel")
                        .debug_selector(|| "nearby-setup-cancel".into())
                        .p(px(8.))
                        .rounded(px(crate::config::corners::CONTROL))
                        .bg(rgb(theme.active))
                        .cursor_pointer()
                        .child(fl!("devices-nearby-cancel"))
                        .on_click(cx.listener(|this, _, _, cx| this.stop_nearby_pairing(cx))),
                )
            });
        frame(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(body)
                .child(footer),
            cx,
        )
    }

    /// The computers found, or why none can be.
    fn nearby_list(&self, setup: &NearbySetup, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let mut list = div()
            .debug_selector(|| "nearby-setup-computers".into())
            .flex()
            .flex_col()
            .gap(px(2.));
        let note = |text: String| div().text_color(rgb(theme.muted)).child(text);
        if !cfg!(target_os = "macos") {
            return list.child(note(fl!("devices-nearby-macos-only")));
        }
        match setup.radio {
            Some(Radio::Off) => return list.child(note(fl!("devices-nearby-bluetooth-off"))),
            Some(Radio::Unauthorized) => {
                return list.child(note(fl!("devices-nearby-bluetooth-allow")));
            }
            Some(Radio::Unsupported) => {
                return list.child(note(fl!("devices-nearby-bluetooth-unsupported")));
            }
            Some(Radio::On | Radio::Unknown) | None => {}
        }
        if setup.scanner.is_none() {
            return list.child(note(fl!("devices-nearby-unavailable")));
        }
        if setup.found.is_empty() {
            return list.child(note(fl!("devices-nearby-looking")));
        }
        for (index, found) in setup.found.iter().enumerate() {
            let peripheral = found.peripheral.clone();
            list = list.child(
                div()
                    .id(("nearby-computer", index))
                    .debug_selector(move || format!("nearby-computer-{index}"))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(crate::config::corners::CONTROL))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(theme.active)))
                    .child(
                        svg()
                            .path("icons/computer.svg")
                            .size(px(self.config.ui.size + 2.))
                            .flex_none()
                            .text_color(rgb(theme.subtext())),
                    )
                    .child(div().flex_1().truncate().child(found.name.clone()))
                    .child(
                        div()
                            .text_color(rgb(theme.muted))
                            .child(fl!("devices-nearby-pair")),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.pair_nearby(&peripheral, cx))),
            );
        }
        list
    }

    /// The code both screens show, and what to do on the other computer.
    fn nearby_code(&self, computer: &Found, code: &str, claimed: bool) -> Div {
        let theme = &self.theme;
        let spaced = match code.split_at_checked(3) {
            Some((first, second)) => format!("{first} {second}"),
            None => code.to_owned(),
        };
        div()
            .debug_selector(|| "nearby-setup-code".into())
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(fl!(
                "devices-nearby-compare",
                computer = computer.name.as_str()
            ))
            .child(
                div()
                    .debug_selector(|| "nearby-setup-digits".into())
                    .text_font(&self.config.terminal)
                    .text_size(px(28.))
                    .line_height(px(36.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(spaced),
            )
            .child(div().text_color(rgb(theme.muted)).child(if claimed {
                fl!("devices-nearby-waiting", computer = computer.name.as_str())
            } else {
                fl!("devices-nearby-claiming", computer = computer.name.as_str())
            }))
    }
}
