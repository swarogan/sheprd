//! Relays shared with this computer by Sheprd on another one, so a relay
//! the user adds once reaches every machine their Sheprd connects to
//! (`docs/relay-plan.md`, Choosing a relay).
//!
//! Each sharing computer is a source, and its offer replaces whatever it
//! shared before, whole: removing a relay there removes it here, and an
//! empty offer withdraws the source. The daemon listens on the relays in
//! its own `[remote] relays` and on every source's, each once. Relays
//! shared here never touch that setting, which stays this computer's own.

mod error;

pub use error::Error;

use serde::{Deserialize, Serialize};
use sheprd_relay::Endpoint;
use std::collections::BTreeMap;

/// The most computers that may share relays with this one.
pub const MAX_SOURCES: usize = 16;
/// The most relays one source may share.
pub const MAX_RELAYS: usize = 16;
/// The longest source name, in bytes.
pub const MAX_SOURCE_BYTES: usize = 128;

/// What Sheprd sends: the relays it listens on, named for the computer
/// it runs on.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub source: String,
    pub relays: Vec<String>,
}

/// Every source's relays, as the daemon keeps them.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Shared {
    sources: BTreeMap<String, Vec<Endpoint>>,
}

impl Shared {
    /// Takes `offer`'s relays in place of what its source shared before.
    /// Returns whether anything changed; refuses an offer it cannot keep
    /// and leaves the rest as it was.
    pub fn offer(&mut self, offer: Offer) -> Result<bool, Error> {
        let source = offer.source.trim();
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(Error::Source);
        }
        let mut relays: Vec<Endpoint> = Vec::new();
        for relay in offer.relays {
            let endpoint = Endpoint::parse(&relay).map_err(|source| Error::Endpoint {
                relay: relay.clone(),
                source,
            })?;
            if relays.len() < MAX_RELAYS && !relays.contains(&endpoint) {
                relays.push(endpoint);
            }
        }
        if relays.is_empty() {
            return Ok(self.sources.remove(source).is_some());
        }
        if !self.sources.contains_key(source) && self.sources.len() >= MAX_SOURCES {
            return Err(Error::TooManySources);
        }
        let before = self.sources.insert(source.to_owned(), relays.clone());
        Ok(before.as_ref() != Some(&relays))
    }

    /// Every source's relays, each once, by source name.
    pub fn endpoints(&self) -> Vec<Endpoint> {
        let mut all: Vec<Endpoint> = Vec::new();
        for endpoint in self.sources.values().flatten() {
            if !all.contains(endpoint) {
                all.push(endpoint.clone());
            }
        }
        all
    }

    /// This copy within bounds, as a file read back may not be.
    pub fn bounded(self) -> Self {
        let sources = self
            .sources
            .into_iter()
            .filter(|(source, relays)| {
                !source.trim().is_empty() && source.len() <= MAX_SOURCE_BYTES && !relays.is_empty()
            })
            .take(MAX_SOURCES)
            .map(|(source, mut relays)| {
                relays.truncate(MAX_RELAYS);
                (source, relays)
            })
            .collect();
        Self { sources }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
