//! The `effects` case: an agent's text at each size, each bubble effect, and
//! one message per pane effect. A loaded transcript plays nothing, so the
//! case plays each effect in turn, as its replay link would, and with a
//! capture path saves a picture part way through each one beside it, named
//! after the effect.

use super::PANE;
use crate::{
    SheprdWindow,
    chat::effects::{Bubble, Screen},
};
use anyhow::Result;
use gpui::{AsyncApp, WindowHandle};
use sheprd_chat_protocol::{Entry, EntryId, EntryKind};
use std::path::{Path, PathBuf};

/// The message with the sizes and the bubbles.
const BUBBLES: EntryId = EntryId(2);
/// The first of the messages with a pane effect each.
const FIRST_SCREEN: u64 = 3;

pub(super) fn entries() -> Vec<Entry> {
    let mut text = String::from(
        "Sizes: :small[small], normal, :big[big], :huge[huge] and :giant[giant] in one line.\n\n", // i18n: data
    );
    for effect in Bubble::ALL {
        text.push_str(&format!(
            ":::{}\nThis paragraph plays **{}**.\n:::\n\n", // i18n: data
            effect.name(),
            effect.name()
        ));
    }
    let mut entries = vec![
        Entry {
            id: EntryId(1),
            kind: EntryKind::User {
                text: "Show me every effect.".into(), // i18n: data
                files: 0,
            },
        },
        Entry {
            id: BUBBLES,
            kind: EntryKind::Agent { text },
        },
    ];
    entries.extend(Screen::ALL.iter().enumerate().map(|(index, effect)| Entry {
        id: EntryId(FIRST_SCREEN + index as u64),
        kind: EntryKind::Agent {
            text: format!("::{}\nThis message plays {}.", effect.name(), effect.name()), // i18n: data
        },
    }));
    entries
}

/// Where the picture of `effect` goes, beside the case's own capture.
fn beside(capture: &Path, effect: &str) -> PathBuf {
    let stem = capture
        .file_stem()
        .map_or_else(|| "effects".into(), |stem| stem.to_string_lossy());
    capture.with_file_name(format!("{stem}-{effect}.png"))
}

/// Scrolls the chat to show its `index`th entry, and keeps it there rather
/// than following the end.
fn reveal(chat: &mut crate::chat::chats::ChatPane, index: usize) {
    chat.list.scroll_to(gpui::ListOffset {
        item_ix: index,
        offset_in_item: gpui::px(0.),
    });
    chat.glide.user_scrolled();
}

/// Plays every effect in turn, capturing each part way through.
pub(super) async fn play(
    handle: WindowHandle<SheprdWindow>,
    capture: Option<&Path>,
    cx: &mut AsyncApp,
) -> Result<()> {
    let timer = cx.background_executor().clone();
    for (order, effect) in Bubble::ALL.iter().enumerate() {
        handle.update(cx, |view, _, cx| {
            if let Some(chat) = view.chats.pane_mut(PANE) {
                reveal(chat, 0);
                chat.effects
                    .replay_bubble(BUBBLES, order, crate::motion::now());
            }
            cx.notify();
        })?;
        timer.timer(effect.length().mul_f32(0.3)).await;
        if let Some(capture) = capture {
            super::save(handle, &beside(capture, effect.name()), cx).await?;
        }
        timer.timer(effect.length()).await;
    }
    for (index, effect) in Screen::ALL.iter().enumerate() {
        let entry = EntryId(FIRST_SCREEN + index as u64);
        handle.update(cx, |view, _, cx| {
            if let Some(chat) = view.chats.pane_mut(PANE) {
                reveal(chat, 2 + index);
                if let Some(body) = chat.prepared.body(entry) {
                    chat.effects
                        .replay_screen(entry, &body, crate::motion::now());
                }
            }
            cx.notify();
        })?;
        timer.timer(effect.length().mul_f32(0.45)).await;
        if let Some(capture) = capture {
            super::save(handle, &beside(capture, effect.name()), cx).await?;
        }
        timer.timer(effect.length().mul_f32(0.6)).await;
    }
    Ok(())
}
