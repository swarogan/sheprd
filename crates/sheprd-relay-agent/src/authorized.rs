//! The desktops allowed to dial this agent, one per line of `authorized`
//! in its state directory:
//!
//! ```text
//! # DIAL_KEY TUNNEL_KEY [LABEL]
//! A3k…  Qm9…  studio
//! ```
//!
//! The dial key is the desktop's P-256 key, SEC1 compressed; the tunnel key
//! its X25519 static key; both base64url without padding. The agent names
//! the dial keys to every relay and admits only the tunnel keys, so a
//! desktop needs both. Blank lines and `#` comments are ignored.

use crate::{Error, Result, files};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sheprd_relay::wire::{KEY_LEN, Key, MAX_DIAL_KEYS};
use std::{fmt::Write as _, path::Path};

pub const FILE: &str = "authorized";
/// As many desktops as a relay takes dial keys for one route.
pub const MAX_ENTRIES: usize = MAX_DIAL_KEYS;
const MAX_BYTES: u64 = 64 * 1024;
/// Labels longer than this are cut, on a character boundary.
pub const MAX_LABEL: usize = 128;
const TUNNEL_LEN: usize = 32;

/// One desktop allowed to dial.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// What the desktop proves to the relay.
    pub dial: Key,
    /// What the desktop's end-to-end handshake must show.
    pub tunnel: [u8; TUNNEL_LEN],
    /// The person's name for the desktop; never used to decide anything.
    pub label: String,
}

impl Entry {
    /// An entry from the base64url keys a desktop gives, and its label.
    /// `None` when either key is not one.
    #[must_use]
    pub fn parse(dial: &str, tunnel: &str, label: &str) -> Option<Self> {
        Some(Self {
            dial: dial_key(dial)?,
            tunnel: URL_SAFE_NO_PAD.decode(tunnel).ok()?.try_into().ok()?,
            label: clean_label(label),
        })
    }
}

/// A dial key: base64url of a P-256 point, SEC1 compressed.
#[must_use]
pub fn dial_key(text: &str) -> Option<Key> {
    let key: [u8; KEY_LEN] = URL_SAFE_NO_PAD.decode(text).ok()?.try_into().ok()?;
    p256::PublicKey::from_sec1_bytes(&key).ok()?;
    Some(key)
}

/// A label on one line, without control characters, at most
/// [`MAX_LABEL`] bytes.
fn clean_label(label: &str) -> String {
    let words = label.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut clean = String::new();
    for character in words.chars().filter(|c| !c.is_control()) {
        if clean.len() + character.len_utf8() > MAX_LABEL {
            break;
        }
        clean.push(character);
    }
    clean
}

/// Every desktop allowed to dial, in the file's order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Authorized {
    entries: Vec<Entry>,
}

impl Authorized {
    /// The file at `state_dir`, or none when it does not exist. Blocking.
    pub fn load(state_dir: &Path) -> Result<Self> {
        let path = state_dir.join(FILE);
        let Some(bytes) = files::read_bounded(&path, MAX_BYTES)? else {
            return Ok(Self::default());
        };
        let text = String::from_utf8_lossy(&bytes);
        Self::parse(&text).map_err(|failure| match failure {
            Parse::Line(line) => Error::AuthorizedLine { line, path },
            Parse::Full => Error::AuthorizedFull { path },
        })
    }

    fn parse(text: &str) -> Result<Self, Parse> {
        let mut authorized = Self::default();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split_whitespace();
            let (Some(dial), Some(tunnel)) = (fields.next(), fields.next()) else {
                return Err(Parse::Line(index + 1));
            };
            let label = fields.collect::<Vec<_>>().join(" ");
            let entry = Entry::parse(dial, tunnel, &label).ok_or(Parse::Line(index + 1))?;
            authorized.insert(entry).map_err(|_| Parse::Full)?;
        }
        Ok(authorized)
    }

    /// Writes the file at `state_dir`. Blocking. Changing the file from
    /// what was read takes [`Authorized::update`] instead.
    pub fn save(&self, state_dir: &Path) -> Result<()> {
        files::write_private(&state_dir.join(FILE), self.render().as_bytes())
    }

    /// Reads the file at `state_dir`, lets `change` edit it, and writes it
    /// back when `change` returns that it changed, all under the state
    /// directory's lock: concurrent `authorize` and `revoke` commands each
    /// see the other's result, so none is lost or undone. Blocking.
    pub fn update(state_dir: &Path, change: impl FnOnce(&mut Self) -> Result<bool>) -> Result<()> {
        let _lock = files::Lock::acquire(state_dir)?;
        let mut authorized = Self::load(state_dir)?;
        if change(&mut authorized)? {
            authorized.save(state_dir)?;
        }
        Ok(())
    }

    fn render(&self) -> String {
        let mut text = String::from("# DIAL_KEY TUNNEL_KEY [LABEL]\n");
        for entry in &self.entries {
            let _ = writeln!(
                text,
                "{} {} {}",
                URL_SAFE_NO_PAD.encode(entry.dial),
                URL_SAFE_NO_PAD.encode(entry.tunnel),
                entry.label
            );
        }
        text
    }

    /// Adds `entry`, replacing any entry with either of its keys, so
    /// authorizing a desktop again updates it.
    pub fn insert(&mut self, entry: Entry) -> Result<(), Full> {
        self.entries
            .retain(|old| old.dial != entry.dial && old.tunnel != entry.tunnel);
        if self.entries.len() >= MAX_ENTRIES {
            return Err(Full);
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Removes the desktop with `dial`; whether there was one.
    pub fn remove(&mut self, dial: &Key) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.dial != *dial);
        self.entries.len() != before
    }

    /// The keys the relays let dial.
    #[must_use]
    pub fn dial_keys(&self) -> Vec<Key> {
        self.entries.iter().map(|entry| entry.dial).collect()
    }

    /// Whether a desktop whose handshake shows `tunnel` may connect.
    #[must_use]
    pub fn admits(&self, tunnel: &[u8; TUNNEL_LEN]) -> bool {
        self.entries.iter().any(|entry| entry.tunnel == *tunnel)
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

/// The file already names [`MAX_ENTRIES`] desktops.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Full;

enum Parse {
    Line(usize),
    Full,
}

#[cfg(test)]
mod tests;
