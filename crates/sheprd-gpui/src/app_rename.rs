//! The one-time offer to move `Herdr.app` to `Sheprd.app`, the name a new
//! install gets. A standalone app renames itself, as its updates would; a
//! Homebrew one is told how to reinstall from the current cask, since moving
//! it would leave Homebrew's receipt stale. The answer is saved, so the app
//! asks once; a moved app is not offered again, since its old path is then
//! only a link.
// i18n: migrated
use crate::state_file;
use gpui::{App, Global, Task, WindowId};
use serde::{Deserialize, Serialize};
use sheprd_updater::Legacy;
use std::path::PathBuf;

/// What the user chose when asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Choice {
    /// Copied the Homebrew reinstall commands.
    Copied,
    Declined,
    Renamed,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    choice: Choice,
}

/// The bundle to offer to move, and whether a window should still ask.
#[derive(Default)]
pub(crate) struct AppRename {
    bundle: Option<Legacy>,
    /// Asked at most once per run, by the first window ready to.
    ask: bool,
    path: Option<PathBuf>,
    /// The window showing the offer, once one asked.
    shown_in: Option<WindowId>,
}

impl Global for AppRename {}

impl AppRename {
    fn path() -> Option<PathBuf> {
        crate::preferences::state_dir().map(|dir| dir.join("app-rename.json"))
    }

    /// Called before starting GPUI, as it reads the disk. Only a release
    /// build asks: a development build is not what the user installed.
    pub(crate) fn load() -> Self {
        if !crate::RELEASE_BUILD {
            return Self::default();
        }
        let path = Self::path();
        let answered = path
            .as_deref()
            .and_then(|path| match state_file::read(path, 4096) {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(%error, "Cannot read the app rename choice");
                    None
                }
            })
            .and_then(|bytes| serde_json::from_slice::<Saved>(&bytes).ok())
            .is_some();
        let bundle = if answered { None } else { Legacy::detect() };
        Self {
            ask: bundle.is_some(),
            bundle,
            path,
            shown_in: None,
        }
    }

    /// An app running from `bundle` that has not been asked yet, saving its
    /// answer at `path`.
    #[cfg(all(test, unix))]
    pub(crate) fn unasked(bundle: Legacy, path: Option<PathBuf>) -> Self {
        Self {
            bundle: Some(bundle),
            ask: true,
            path,
            shown_in: None,
        }
    }

    /// Closing the window showing the offer, or quitting while it is shown,
    /// answers "Not now", as dismissing it does, so the app asks only once.
    pub(crate) fn install(self, cx: &mut App) {
        cx.set_global(self);
        cx.on_window_closed(|cx, closed| {
            if Self::showing(cx).is_some_and(|window| window == closed) {
                Self::choose(Choice::Declined, cx).detach();
            }
        })
        .detach();
        cx.on_app_quit(|cx| {
            let save = Self::showing(cx).map(|_| Self::choose(Choice::Declined, cx));
            async move {
                if let Some(save) = save {
                    save.await;
                }
            }
        })
        .detach();
    }

    /// The bundle to move, while the offer stands.
    pub(crate) fn bundle(cx: &App) -> Option<&Legacy> {
        cx.try_global::<Self>()
            .and_then(|rename| rename.bundle.as_ref())
    }

    /// Whether `window` should ask now; claims the question so no other
    /// window asks too.
    pub(crate) fn take_ask(window: WindowId, cx: &mut App) -> bool {
        let ask = cx.try_global::<Self>().is_some_and(|rename| rename.ask);
        if ask {
            let rename = cx.global_mut::<Self>();
            rename.ask = false;
            rename.shown_in = Some(window);
        }
        ask
    }

    /// The window showing the offer, while it is unanswered.
    fn showing(cx: &App) -> Option<WindowId> {
        let rename = cx.try_global::<Self>()?;
        rename.bundle.as_ref().and(rename.shown_in)
    }

    /// Records the user's answer, saved off the UI thread, and withdraws the
    /// offer: a failed move is not offered again either, so the app never
    /// asks twice; the user can still rename the app in Finder.
    pub(crate) fn choose(choice: Choice, cx: &mut App) -> Task<()> {
        if !cx.has_global::<Self>() {
            return Task::ready(());
        }
        let rename = cx.global_mut::<Self>();
        rename.ask = false;
        if rename.bundle.take().is_none() {
            return Task::ready(());
        }
        let Some(path) = rename.path.clone() else {
            return Task::ready(());
        };
        cx.background_executor().spawn(async move {
            if let Err(error) = state_file::write(&path, &Saved { choice }) {
                tracing::warn!(%error, "Cannot save the app rename choice");
            }
        })
    }
}

#[cfg(all(test, unix))]
mod tests;
