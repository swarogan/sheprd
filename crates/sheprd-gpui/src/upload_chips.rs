//! Dropped files on their way to another host, as a chat composer and a
//! herdr-core terminal show them: a chip for each that fills as its bytes go
//! out, in the order they are sent, and a line with the batch's bytes.
//! The worker that copies them reports through a [`Reporter`]; the view
//! reads the latest count from the [`Upload`], never a queue of reports, so
//! a fast transfer cannot flood the UI.
// i18n: migrated

use crate::{
    config::{Theme, corners, mix},
    i18n::fl,
    notifications::safe_text,
};
use gpui::{prelude::*, *};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

/// One drop's files on their way to a host.
pub(crate) struct Upload {
    names: Vec<String>,
    progress: Arc<Progress>,
}

/// The copying worker's side of an [`Upload`].
#[derive(Clone)]
pub(crate) struct Reporter(Arc<Progress>);

#[derive(Default)]
struct Progress {
    cancelled: AtomicBool,
    done: AtomicBool,
    sent: AtomicU64,
    /// Each file's size, in sending order, once the worker has read them.
    sizes: OnceLock<Vec<u64>>,
}

/// Where one file of an upload is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Sending {
    /// Not started; its size once known.
    Waiting(Option<u64>),
    /// This share of its bytes is out.
    Partly(f32),
    Sent,
}

impl Upload {
    pub(crate) fn new(paths: &[PathBuf]) -> Self {
        let names = paths
            .iter()
            .map(|path| {
                path.file_name()
                    .map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy())
                    .into_owned()
            })
            .collect();
        Self {
            names,
            progress: Arc::default(),
        }
    }

    pub(crate) fn reporter(&self) -> Reporter {
        Reporter(self.progress.clone())
    }

    /// Each file's name and how far it has come.
    pub(crate) fn files(&self) -> impl Iterator<Item = (&str, Sending)> {
        let states = self.progress.sizes.get().map_or_else(
            || vec![Sending::Waiting(None); self.names.len()],
            |sizes| split(self.sent(), sizes),
        );
        self.names.iter().map(String::as_str).zip(states)
    }

    /// Bytes sent and the batch's total, once the sizes are known.
    pub(crate) fn bytes(&self) -> Option<(u64, u64)> {
        let total = self.progress.sizes.get()?.iter().sum();
        Some((self.sent().min(total), total))
    }

    /// Bytes out so far, to tell whether the chips need drawing again.
    pub(crate) fn sent(&self) -> u64 {
        self.progress.sent.load(Ordering::Acquire)
    }

    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn cancel(&self) {
        self.progress.cancelled.store(true, Ordering::Release);
    }

    pub(crate) fn is(&self, reporter: &Reporter) -> bool {
        Arc::ptr_eq(&self.progress, &reporter.0)
    }
}

impl Reporter {
    /// Reads each file's size. Blocking: on the worker, before sending. A
    /// file that cannot be read fails the copy itself; until then it counts
    /// as empty.
    pub(crate) fn measure(&self, paths: &[PathBuf]) {
        let _ = self.0.sizes.set(
            paths
                .iter()
                .map(|path| std::fs::metadata(path).map_or(0, |metadata| metadata.len()))
                .collect(),
        );
    }

    /// The bytes sent so far, as the copy reports them.
    pub(crate) fn sent(&self, bytes: u64) {
        self.0.sent.store(bytes, Ordering::Release);
    }

    pub(crate) fn sent_bytes(&self) -> u64 {
        self.0.sent.load(Ordering::Acquire)
    }

    /// Whether the sizes have been read, so the chips can show them.
    pub(crate) fn measured(&self) -> bool {
        self.0.sizes.get().is_some()
    }

    /// Whether the user cancelled from a chip, even after the copy finished.
    pub(crate) fn was_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn finish(&self) {
        self.0.done.store(true, Ordering::Release);
    }

    pub(crate) fn finished(&self) -> bool {
        self.0.done.load(Ordering::Acquire)
    }

    /// Set when the user cancels from a chip; the copy stops at its next check.
    pub(crate) fn cancelled(&self) -> &AtomicBool {
        &self.0.cancelled
    }
}

/// Spreads `sent` bytes over files sent one after another, in order. An
/// empty file is sent once every file before it is.
pub(crate) fn split(sent: u64, sizes: &[u64]) -> Vec<Sending> {
    let mut before = 0u64;
    sizes
        .iter()
        .map(|&size| {
            let start = before;
            before = before.saturating_add(size);
            if sent >= before {
                Sending::Sent
            } else if sent > start {
                Sending::Partly((sent - start) as f32 / size as f32)
            } else {
                Sending::Waiting(Some(size))
            }
        })
        .collect()
}

/// Bytes sent and the total over `uploads` whose sizes are known.
pub(crate) fn totals<'a>(uploads: impl IntoIterator<Item = &'a Upload>) -> Option<(u64, u64)> {
    let (sent, total) = uploads
        .into_iter()
        .filter_map(Upload::bytes)
        .fold((0u64, 0u64), |(sent, total), (more, of)| {
            (sent.saturating_add(more), total.saturating_add(of))
        });
    (total > 0).then_some((sent, total))
}

/// "Copying to devbox · 3.4 MB of 12.8 MB".
pub(crate) fn status(host: &str, sent: u64, total: u64) -> String {
    fl!(
        "upload-chips-progress",
        host = host.to_owned(),
        sent = crate::automations::bytes(sent),
        total = crate::automations::bytes(total)
    )
}

/// Whether a file's name says it holds a picture.
pub(crate) fn is_picture(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| {
            sheprd_chat_protocol::ImageType::from_mime(&format!(
                "image/{}",
                extension.to_ascii_lowercase()
            ))
        })
        .is_some()
}

/// A file's chip: its mark, name, how far it has come, and `cancel`, the
/// × button the caller wires; a bar along its foot fills until it is sent.
/// `id` names it for tests, as `<id>-<index>`.
pub(crate) fn chip(
    theme: &Theme,
    text_size: Pixels,
    id: &'static str,
    index: usize,
    (name, sending): (&str, Sending),
    cancel: Stateful<Div>,
) -> Div {
    let green = theme.ink(theme.palette[2]);
    let mark = match sending {
        Sending::Sent => svg().path("icons/check.svg").text_color(rgb(green)),
        _ => svg()
            .path(if is_picture(name) {
                "icons/zoom.svg"
            } else {
                "icons/file.svg"
            })
            .text_color(rgb(theme.subtext())),
    };
    let detail = match sending {
        Sending::Sent => None,
        Sending::Partly(share) => Some(fl!(
            "upload-chips-percent",
            percent = (share.clamp(0., 1.) * 100.).floor().to_string()
        )),
        Sending::Waiting(Some(size)) => Some(fl!(
            "upload-chips-waiting-size",
            size = crate::automations::bytes(size)
        )),
        Sending::Waiting(None) => Some(fl!("upload-chips-waiting")),
    };
    let share = match sending {
        Sending::Sent => None,
        Sending::Partly(share) => Some(share.clamp(0., 1.)),
        Sending::Waiting(_) => Some(0.),
    };
    div()
        .debug_selector(move || format!("{id}-{index}"))
        .relative()
        .overflow_hidden()
        .flex()
        .items_center()
        .gap(px(5.))
        .pl(px(8.))
        .pr(px(2.))
        .h(px(24.))
        .max_w(px(260.))
        .rounded(px(corners::CONTROL))
        .bg(rgb(theme.active))
        .text_size(text_size)
        .text_color(rgb(theme.foreground))
        .child(mark.flex_none().size(px(11.)))
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(matches!(sending, Sending::Waiting(_)), |name| {
                    name.text_color(rgb(theme.muted))
                })
                .child(safe_text(name, 200)),
        )
        .children(detail.map(|detail| div().flex_none().text_color(rgb(theme.muted)).child(detail)))
        .child(cancel)
        .children(share.map(|share| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(2.))
                .bg(rgb(mix(theme.active, theme.background, 40)))
                .child(div().h_full().w(relative(share)).bg(rgb(theme.primary())))
        }))
}

/// A chip's × button, named `<id>-<index>`, for the caller's click handler.
pub(crate) fn cancel_button(theme: &Theme, id: &'static str, index: usize) -> Stateful<Div> {
    let hover = theme.background;
    div()
        .id((id, index))
        .debug_selector(move || format!("{id}-{index}"))
        .flex_none()
        .size(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(corners::SMALL))
        .cursor_pointer()
        .hover(move |button| button.bg(rgb(hover)))
        .child(
            svg()
                .path("icons/close.svg")
                .size(px(9.))
                .text_color(rgb(theme.subtext())),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

#[cfg(test)]
impl Upload {
    /// An upload `sent` bytes into files of `sizes`, as a worker leaves it.
    pub(crate) fn sample(names: &[&str], sizes: Vec<u64>, sent: u64) -> Self {
        let upload = Self::new(&names.iter().map(PathBuf::from).collect::<Vec<_>>());
        let _ = upload.progress.sizes.set(sizes);
        upload.progress.sent.store(sent, Ordering::Release);
        upload
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.progress.cancelled.load(Ordering::Acquire)
    }
}
