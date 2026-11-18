//! Dropped files on their way to the chat's host: a chip for each, among
//! the attachments, and a line in the composer's footer for the whole copy.

use super::super::chats::ChatPane;
use crate::{SheprdWindow, upload_chips};
use gpui::{prelude::*, *};

impl SheprdWindow {
    /// A chip for every file still being copied, in sending order.
    pub(super) fn render_chat_uploads(
        &self,
        chat: &ChatPane,
        small: Pixels,
        cx: &mut Context<Self>,
    ) -> Vec<Div> {
        let theme = &self.theme;
        chat.uploads
            .iter()
            .flat_map(|upload| upload.files())
            .enumerate()
            .map(|(index, file)| {
                let pane_id = chat.target.pane_id.clone();
                let cancel = upload_chips::cancel_button(theme, "chat-upload-cancel", index)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.cancel_chat_upload(&pane_id, index, cx);
                    }));
                upload_chips::chip(theme, small, "chat-upload", index, file, cancel)
            })
            .collect()
    }

    /// "Copying to devbox · 3.4 MB of 12.8 MB", once the sizes are known.
    pub(super) fn render_chat_upload_status(&self, chat: &ChatPane, host: &str) -> Option<Div> {
        let (sent, total) = upload_chips::totals(&chat.uploads)?;
        Some(
            div()
                .debug_selector(|| "chat-upload-status".into())
                .min_w_0()
                .truncate()
                .text_color(rgb(self.theme.subtext()))
                .child(upload_chips::status(host, sent, total)),
        )
    }
}
