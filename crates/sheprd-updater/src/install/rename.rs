//! An app still installed as `Herdr.app`, which a new install names
//! `Sheprd.app`. A standalone bundle can move when the user asks, rather than
//! at its next update: the same move an update makes, with the same lock, the
//! same hidden link at the old path, and the same restart. A Homebrew bundle
//! never moves itself, as its receipt would go stale; the user reinstalls it
//! from the current cask instead.
use super::{Installation, Mode, bundle, io, location};
use crate::{Error, Result, brew};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// The cask a new Homebrew install comes from.
const CURRENT_CASK: &str = "sheprd-labs/tap/sheprd";

const MOVED: brew::Alerts = brew::Alerts {
    stuck: "Sheprd did not quit in time to restart. Quit Sheprd, then open Sheprd from Applications.",
    failed: "Sheprd was renamed but could not restart. Open Sheprd from Applications.",
};

/// How this app, running from `Herdr.app`, reaches the current name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Legacy {
    /// The app can rename itself.
    Standalone(LegacyBundle),
    /// Homebrew installed it; reinstalling from the current cask renames it.
    Homebrew(HomebrewBundle),
}

/// A standalone `Herdr.app`, and where it moves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyBundle {
    bundle: PathBuf,
    renamed: PathBuf,
}

/// A `Herdr.app` that a Homebrew cask's receipt names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomebrewBundle {
    bundle: PathBuf,
    cask: &'static str,
}

impl Legacy {
    /// What this app runs from, when it still has the old name. Blocking: it
    /// reads the bundle's folder and Homebrew's receipts. Always `None` off
    /// macOS.
    pub fn detect() -> Option<Self> {
        if !cfg!(target_os = "macos") {
            return None;
        }
        let executable = std::env::current_exe().ok()?;
        let bundle = location::mac_bundle(&executable).ok()?;
        Self::at(&bundle, brew::recording_cask)
    }

    /// `bundle`, when it is a real `Herdr.app` folder. `cask` names the
    /// Homebrew cask whose receipt records it, if any. A standalone bundle is
    /// offered the move only while nothing stands at `Sheprd.app` beside it,
    /// which is never replaced. Blocking.
    pub fn at(bundle: &Path, cask: impl FnOnce(&Path) -> Option<&'static str>) -> Option<Self> {
        let renamed = bundle::renamed(bundle)?;
        if !fs::symlink_metadata(bundle).is_ok_and(|meta| meta.is_dir()) {
            return None;
        }
        if let Some(cask) = cask(bundle) {
            return Some(Self::Homebrew(HomebrewBundle {
                bundle: bundle.to_owned(),
                cask,
            }));
        }
        absent(&renamed).unwrap_or(false).then(|| {
            Self::Standalone(LegacyBundle {
                bundle: bundle.to_owned(),
                renamed,
            })
        })
    }

    /// Where the app is now.
    pub fn path(&self) -> &Path {
        match self {
            Self::Standalone(legacy) => &legacy.bundle,
            Self::Homebrew(homebrew) => &homebrew.bundle,
        }
    }
}

impl HomebrewBundle {
    /// The commands that reinstall the app under the current name, one per
    /// line. Uninstalling removes only the bundle: settings, sign-ins, saved
    /// tabs, and history live outside it, and the casks have no `zap`.
    pub fn commands(&self) -> String {
        format!(
            "brew uninstall --cask {}\nbrew install {CURRENT_CASK}",
            self.cask
        )
    }
}

impl LegacyBundle {
    /// Where the app moves to.
    pub fn renamed(&self) -> &Path {
        &self.renamed
    }

    /// Moves the bundle, links its old path to the new one, and arms a restart
    /// from the new path once this process exits. On `Ok` the caller quits.
    /// Blocking; run it off the UI thread.
    pub fn rename_and_relaunch(&self) -> Result<()> {
        let uid = location::effective_uid()?;
        let installation = Installation {
            mode: Mode::Mac,
            destination: self.bundle.clone(),
            executable: std::env::current_exe().map_err(io)?,
            uid,
        };
        // An update replacing this bundle holds the same lock.
        let _lease = location::lock(&installation)?;
        self.rename()?;
        bundle::executable(&self.renamed)
            .map_err(|_| Error::RelaunchMissing(self.renamed.join("Contents/MacOS")))?;
        brew::schedule_relaunch(
            std::process::id(),
            &self.renamed,
            crate::APP_ID,
            "/usr/bin:/bin",
            brew::RELAUNCH,
            &MOVED,
        )
    }

    /// The move itself. The old path must keep resolving, since a core started
    /// from it outlives the app, so a failed link puts the bundle back.
    pub(crate) fn rename(&self) -> Result<()> {
        if !absent(&self.renamed).map_err(io)? {
            return Err(io(ErrorKind::AlreadyExists.into()));
        }
        fs::rename(&self.bundle, &self.renamed).map_err(io)?;
        if let Err(error) = bundle::link_legacy(&self.bundle) {
            if let Err(rollback) = fs::rename(&self.renamed, &self.bundle) {
                tracing::error!(
                    category = "updater",
                    %rollback,
                    "could not move the app back after linking its old path failed"
                );
            }
            return Err(error);
        }
        Ok(())
    }
}

fn absent(path: &Path) -> std::io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
