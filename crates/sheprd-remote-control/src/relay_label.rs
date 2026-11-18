//! What a person calls each relay in `[remote] relays`: a name, a line of
//! description, and tags, so `sheprd remote status` and the phone's
//! computer page tell relays apart ("fra3", "not for public use").
//!
//! A label is the user's own words for their own list. Relays never see
//! it: the daemon reports it on its control socket and to paired devices
//! over `GET /api/relay`, never in the nearby offer, whose sealed bytes
//! stay as small as the vectors fix them.

mod error;

pub use error::Error;

use serde::{Deserialize, Serialize};

/// The longest name, in characters.
pub const NAME_MAX: usize = 64;
/// The longest description, in characters.
pub const DESCRIPTION_MAX: usize = 256;
/// The longest tag, in characters.
pub const TAG_MAX: usize = 32;
/// The most tags one relay has.
pub const TAGS_MAX: usize = 8;

/// A relay's name, description, and tags; each may be left out.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RelayLabel {
    // keep-sorted start
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    // keep-sorted end
}

impl RelayLabel {
    /// Whether the label says nothing.
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.description.is_none() && self.tags.is_empty()
    }

    /// Refuses a blank or overlong field, a control character, which
    /// would break the one line each field is shown on, or too many tags.
    pub fn check(&self) -> Result<(), Error> {
        if let Some(name) = &self.name {
            text(name, "name", NAME_MAX)?;
        }
        if let Some(description) = &self.description {
            text(description, "description", DESCRIPTION_MAX)?;
        }
        if self.tags.len() > TAGS_MAX {
            return Err(Error::Tags);
        }
        self.tags
            .iter()
            .try_for_each(|tag| text(tag, "tag", TAG_MAX))
    }

    /// The daemon's options that carry this label, after its `--relay`.
    pub fn args(&self) -> impl Iterator<Item = (&'static str, &str)> {
        let name = self.name.as_deref().map(|name| ("--relay-name", name));
        let description = self
            .description
            .as_deref()
            .map(|description| ("--relay-description", description));
        let tags = self.tags.iter().map(|tag| ("--relay-tag", tag.as_str()));
        name.into_iter().chain(description).chain(tags)
    }
}

fn text(value: &str, field: &'static str, max: usize) -> Result<(), Error> {
    if value.trim().is_empty() {
        return Err(Error::Blank { field });
    }
    if value.chars().any(char::is_control) {
        return Err(Error::Control { field });
    }
    if value.chars().count() > max {
        return Err(Error::Length { field, max });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
