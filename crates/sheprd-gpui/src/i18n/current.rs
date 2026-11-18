//! The catalog the process shows, and the direction its locale lays the
//! chrome out in. Tests keep one per thread: each GPUI test runs on its own,
//! and one switching to a pseudo-locale must not change the text or the
//! layout another test reads.

use super::FILES;
use sheprd_i18n::{Catalog, Direction, Locale};
use std::sync::{Arc, LazyLock};

/// English, until the app picks the setting's locale.
static DEFAULT: LazyLock<Option<Arc<Catalog>>> =
    LazyLock::new(|| load(&Locale::FALLBACK).map(Arc::new));

fn load(locale: &Locale) -> Option<Catalog> {
    Catalog::new(FILES, locale.clone())
        .inspect_err(|error| tracing::warn!(%error, %locale, "Could not load messages"))
        .ok()
}

#[cfg(not(test))]
static CURRENT: std::sync::RwLock<Option<Arc<Catalog>>> = std::sync::RwLock::new(None);

/// Whether the shown locale runs right to left. Layout reads it for every
/// element it mirrors, so it is kept apart from the catalog's lock.
#[cfg(not(test))]
static RTL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(not(test))]
pub(super) fn direction() -> Direction {
    match RTL.load(std::sync::atomic::Ordering::Relaxed) {
        true => Direction::Rtl,
        false => Direction::Ltr,
    }
}

#[cfg(not(test))]
pub(super) fn catalog() -> Option<Arc<Catalog>> {
    CURRENT
        .read()
        .ok()
        .and_then(|current| current.clone())
        .or_else(|| DEFAULT.clone())
}

/// Keeps the catalog shown so far when `locale`'s cannot load.
#[cfg(not(test))]
pub(super) fn replace(locale: &Locale) {
    let Some(catalog) = load(locale) else {
        return;
    };
    RTL.store(
        catalog.locale().direction() == Direction::Rtl,
        std::sync::atomic::Ordering::Relaxed,
    );
    if let Ok(mut current) = CURRENT.write() {
        *current = Some(Arc::new(catalog));
    }
}

#[cfg(test)]
thread_local! {
    static CURRENT: std::cell::RefCell<Option<Arc<Catalog>>> = const { std::cell::RefCell::new(None) };
    /// Each catalog this thread has loaded. Tests switch locales once per
    /// message they check, and parsing every file again for each switch was
    /// most of their time.
    static LOADED: std::cell::RefCell<Vec<(Locale, Arc<Catalog>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(super) fn catalog() -> Option<Arc<Catalog>> {
    CURRENT
        .with_borrow(Clone::clone)
        .or_else(|| DEFAULT.clone())
}

#[cfg(test)]
pub(super) fn direction() -> Direction {
    catalog().map_or(Direction::Ltr, |catalog| catalog.locale().direction())
}

#[cfg(test)]
pub(super) fn replace(locale: &Locale) {
    let cached = LOADED.with_borrow(|loaded| {
        loaded
            .iter()
            .find(|(loaded, _)| loaded == locale)
            .map(|(_, catalog)| catalog.clone())
    });
    let Some(catalog) = cached.or_else(|| {
        let catalog = Arc::new(load(locale)?);
        LOADED.with_borrow_mut(|loaded| loaded.push((locale.clone(), catalog.clone())));
        Some(catalog)
    }) else {
        return;
    };
    CURRENT.set(Some(catalog));
}
