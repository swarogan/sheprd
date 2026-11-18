//! Stand-ins for Homebrew off Unix, and for the installer on platforms that
//! ship no standalone updater. `release::target()` returns `None` there, so
//! `Updater::start` stays in `State::Disabled` and none of this is ever
//! reached; it exists so the service, its states, and the update panel keep
//! one definition across targets. Windows has its own installer (`windows`).

/// Homebrew is macOS-only, so no cask can own this installation.
pub(super) mod brew {
    use super::super::{Error, Result};
    use std::{path::Path, sync::atomic::AtomicBool};

    /// No cask is ever detected, so this type has no values.
    pub(crate) enum Cask {}

    pub(crate) fn detect(_bundle: &Path, _uid: u32) -> Option<Cask> {
        None
    }

    pub(crate) fn upgrade(
        _cask: &Cask,
        _current: &str,
        _expected: &str,
        _cancel: &AtomicBool,
        _progress: impl FnMut(crate::Progress),
    ) -> Result<String> {
        Err(Error::UnsupportedTarget)
    }

    pub(crate) fn relaunch(_cask: &Cask) -> Result<()> {
        Err(Error::UnsupportedTarget)
    }
}

/// Staging, replacement, and restart all require the POSIX installer.
#[cfg(not(windows))]
pub(super) mod install {
    use super::super::{Error, Result, release};
    use std::{
        ffi::OsString,
        path::{Path, PathBuf},
        process::ExitCode,
        sync::atomic::AtomicBool,
    };

    /// Nothing is ever staged, so this type has no values.
    pub(crate) enum Prepared {}

    /// No restart is ever scheduled, so this type has no values.
    pub(crate) enum RestartGuard {}

    impl RestartGuard {
        pub(crate) fn commit(&mut self) -> Result<()> {
            Err(Error::UnsupportedTarget)
        }
    }

    pub(crate) fn effective_uid() -> Result<u32> {
        Err(Error::UnsupportedTarget)
    }

    pub(crate) fn mac_bundle(_executable: &Path) -> Result<PathBuf> {
        Err(Error::NotHerdrBundle)
    }

    pub(crate) fn prepare(
        _offer: &release::Offer,
        _cancel: &AtomicBool,
        _progress: impl FnMut(u64, u64),
    ) -> Result<Prepared> {
        Err(Error::UnsupportedTarget)
    }

    pub(crate) fn install_and_restart(
        _prepared: Prepared,
        _cancel: &AtomicBool,
    ) -> Result<RestartGuard> {
        Err(Error::UnsupportedTarget)
    }

    /// The helper is spawned by the POSIX installer only, so nothing here
    /// recognizes its arguments and the GUI always starts normally.
    pub(crate) fn run_helper(_args: &[OsString]) -> Option<ExitCode> {
        None
    }
}

/// Only a macOS bundle carries the legacy `Herdr.app` name, so none is ever
/// found here and these types have no values. `Legacy` keeps the macOS
/// variants so the app's matches compile on every target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Legacy {
    Standalone(LegacyBundle),
    Homebrew(HomebrewBundle),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LegacyBundle {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HomebrewBundle {}

impl Legacy {
    pub fn detect() -> Option<Self> {
        None
    }

    pub fn at(
        _bundle: &std::path::Path,
        _cask: impl FnOnce(&std::path::Path) -> Option<&'static str>,
    ) -> Option<Self> {
        None
    }

    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::Standalone(legacy) => match *legacy {},
            Self::Homebrew(homebrew) => match *homebrew {},
        }
    }
}

impl LegacyBundle {
    pub fn renamed(&self) -> &std::path::Path {
        match *self {}
    }

    pub fn rename_and_relaunch(&self) -> super::Result<()> {
        match *self {}
    }
}

impl HomebrewBundle {
    pub fn commands(&self) -> String {
        match *self {}
    }
}
