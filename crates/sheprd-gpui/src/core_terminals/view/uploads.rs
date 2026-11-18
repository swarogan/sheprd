//! Files dropped on a terminal on another host, while they are copied
//! there before their paths are pasted: the chat's chips, in a card along
//! the pane's foot. A chip's × cancels the drop, as Escape does.

use super::CoreTerminalView;
use crate::{config::corners, fonts::StyledFont, upload_chips};
use gpui::{prelude::*, *};

/// The widest the card grows on a wide pane.
const MAX_WIDTH: f32 = 560.;

impl CoreTerminalView {
    pub(super) fn render_uploads(&self, cx: &mut Context<Self>) -> Option<Div> {
        let chrome = &self.look.chrome;
        let theme = &chrome.theme;
        let small = px((chrome.ui.size - 1.).max(10.));
        let (chips, status) = self.with_upload(|upload, host| {
            let chips: Vec<Div> = upload
                .files()
                .enumerate()
                .map(|(index, file)| {
                    let cancel =
                        upload_chips::cancel_button(theme, "terminal-upload-cancel", index)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                if this.cancel_paste() {
                                    cx.notify();
                                }
                            }));
                    upload_chips::chip(theme, small, "terminal-upload", index, file, cancel)
                })
                .collect();
            let status = host.zip(upload.bytes()).map(|(host, (sent, total))| {
                upload_chips::status(&crate::notifications::safe_text(host, 80), sent, total)
            });
            (chips, status)
        })?;
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .p(px(6.))
                .child(
                    div()
                        .id("terminal-uploads")
                        .debug_selector(|| "terminal-uploads".into())
                        .occlude()
                        .max_w(px(MAX_WIDTH))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .p(px(8.))
                        .rounded(px(corners::PANEL))
                        .border_1()
                        .border_color(rgb(theme.active))
                        .bg(rgb(theme.surface))
                        .text_font(&chrome.ui)
                        .text_size(small)
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                        .on_scroll(|_, _, cx| cx.stop_propagation())
                        .child(div().flex().flex_wrap().gap(px(6.)).children(chips))
                        .children(status.map(|status| {
                            div()
                                .debug_selector(|| "terminal-upload-status".into())
                                .px(px(2.))
                                .truncate()
                                .text_color(rgb(theme.subtext()))
                                .child(status)
                        })),
                ),
        )
    }
}
