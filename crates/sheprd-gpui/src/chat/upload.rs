//! Files dropped on a chat whose agent runs on another host, while they are
//! copied there (`crate::upload_chips` draws them). The whole batch can be
//! cancelled, and the copies join the prompt only once every file arrived.

use super::chats::ChatInput;
use crate::upload_chips::{Reporter, Upload};
use crate::{
    SheprdWindow,
    i18n::{Localize, fl},
};
use gpui::{AppContext as _, Context, Window};
use sheprd_chat_protocol::MAX_ATTACHMENTS;
use std::{path::PathBuf, time::Duration};

/// How often the chips catch up with the bytes sent, while any are.
const REFRESH: Duration = Duration::from_millis(100);

impl SheprdWindow {
    /// Copies files to `host` off the UI thread, then attaches the copies.
    pub(super) fn upload_chat_files(
        &mut self,
        pane_id: &str,
        host: sheprd_client::ConnectTarget,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.chats.pane_mut(pane_id) else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        if paths
            .len()
            .saturating_add(pane.attachments.len())
            .saturating_add(pane.uploading)
            > MAX_ATTACHMENTS
        {
            pane.error = Some(fl!("chat-files-too-many", count = MAX_ATTACHMENTS));
            self.redraw_terminal(cx);
            return;
        }
        let target = pane.target.clone();
        let upload = Upload::new(&paths);
        let progress = upload.reporter();
        pane.uploading += paths.len();
        pane.uploads.push(upload);
        let count = paths.len();
        let upload_host = host.clone();
        let worker = progress.clone();
        let upload = cx.background_spawn(async move {
            worker.measure(&paths);
            let uploaded = crate::core_terminals::files::upload(
                &upload_host,
                &paths,
                worker.cancelled(),
                |sent, _| worker.sent(sent),
            );
            worker.finish();
            uploaded
        });
        Self::refresh_chat_upload(progress.clone(), cx);
        let pane_id = pane_id.to_owned();
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |this, cx| {
            let uploaded = upload.await;
            let accepted = this
                .update_in(cx, |this, window, cx| {
                    let Some(pane) = this.chats.pane_mut(&pane_id) else {
                        return false;
                    };
                    pane.uploads.retain(|upload| !upload.is(&progress));
                    if pane.target != target {
                        return false;
                    }
                    pane.uploading = pane.uploading.saturating_sub(count);
                    match &uploaded {
                        // The user asked for it, so there is nothing to
                        // report, and copies that arrived before the click
                        // are removed rather than attached.
                        _ if progress.was_cancelled() => {
                            this.redraw_terminal(cx);
                            false
                        }
                        Ok(remote) => {
                            if pane.input() != ChatInput::Composer
                                || remote.len().saturating_add(pane.attachments.len())
                                    > MAX_ATTACHMENTS
                            {
                                return false;
                            }
                            let remote = remote.iter().map(PathBuf::from).collect();
                            this.add_chat_files(&pane_id, remote, window, cx);
                            true
                        }
                        Err(error) => {
                            pane.error =
                                Some(fl!("chat-files-not-attached", error = error.localize()));
                            this.redraw_terminal(cx);
                            false
                        }
                    }
                })
                .unwrap_or(false);
            if !accepted && let Ok(paths) = uploaded {
                executor
                    .spawn(async move {
                        if let Err(error) = crate::core_terminals::files::cleanup(&host, &paths) {
                            tracing::warn!(%error, "Could not remove unclaimed chat attachments");
                        }
                    })
                    .detach();
            }
        })
        .detach();
        self.redraw_terminal(cx);
    }

    /// Stops copying the batch the file at `index` of the chat's uploads
    /// belongs to. Files already on the host are removed there.
    pub(super) fn cancel_chat_upload(
        &mut self,
        pane_id: &str,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(upload) = self.chats.pane(pane_id).and_then(|pane| {
            let mut first = 0;
            pane.uploads.iter().find(|upload| {
                first += upload.len();
                index < first
            })
        }) else {
            return;
        };
        upload.cancel();
        self.redraw_terminal(cx);
    }

    /// Redraws the chat while the copy runs, at most every [`REFRESH`] and
    /// only when the chips changed: the sizes became known, or more bytes
    /// were sent.
    fn refresh_chat_upload(progress: Reporter, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut seen = (false, 0);
            while !progress.finished() {
                cx.background_executor().timer(REFRESH).await;
                let now = (progress.measured(), progress.sent_bytes());
                if now == seen {
                    continue;
                }
                seen = now;
                if this
                    .update(cx, |this, cx| this.redraw_terminal(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
}
