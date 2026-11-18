//! What an entry holds, and the shapes calls take and return.
use crate::{Error, Result};
use serde::Serialize;
use std::{fmt, str::FromStr};

/// What an entry is, so a reader can tell a rule from a passing remark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// How to build, test, or run something here.
    Command,
    /// A choice that was made, and why.
    Decision,
    /// Something true about the code or its setup.
    #[default]
    Fact,
    /// A trap: what failed, and what works instead.
    Gotcha,
    /// Where work stands, for whoever picks it up next.
    Handoff,
    /// How the person wants things done.
    Preference,
}

/// Every kind, in the order help text names them.
pub const KINDS: &[Kind] = &[
    Kind::Fact,
    Kind::Gotcha,
    Kind::Command,
    Kind::Decision,
    Kind::Preference,
    Kind::Handoff,
];

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Decision => "decision",
            Self::Fact => "fact",
            Self::Gotcha => "gotcha",
            Self::Handoff => "handoff",
            Self::Preference => "preference",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Kind {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        KINDS
            .iter()
            .copied()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| Error::Kind(text.to_owned()))
    }
}

/// Which entries a call sees: the repository's, and one branch's when named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope<'a> {
    /// Every entry, of the repository and of each branch.
    All,
    /// The repository's entries, and those of `branch` when there is one.
    Here { branch: Option<&'a str> },
}

/// An entry to keep. With a `name`, an entry of that name in the same scope
/// is updated in place rather than added beside.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct New<'a> {
    /// The branch it is about; `None` for the whole repository.
    pub branch: Option<&'a str>,
    pub name: Option<&'a str>,
    pub kind: Kind,
    /// One or two sentences: what listings, searches, and recall show.
    pub text: &'a str,
    /// The longer story, shown only when the entry is read on its own.
    pub details: Option<&'a str>,
    /// Pin it: shown first, never evicted. An update that does not pin
    /// leaves a pin in place.
    pub pinned: bool,
}

/// One remembered entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub id: i64,
    /// The branch it is about; `None` for the whole repository.
    pub branch: Option<String>,
    pub name: Option<String>,
    pub kind: Kind,
    pub text: String,
    pub details: Option<String>,
    pub pinned: bool,
    /// How often it was read on its own or found by a search.
    pub uses: i64,
    /// Unix seconds.
    pub created: i64,
    /// Unix seconds it was last added again, read, or found by a search.
    pub used: i64,
    /// Unix seconds its text last changed.
    pub updated: i64,
}

impl Entry {
    /// One line for a listing or a prompt: ID, kind, pin, branch, name, and
    /// text, cut to about `max` characters; `[+details]` marks details.
    ///
    /// Entries are untrusted: any agent may have written one, and these
    /// lines reach other agents' prompts. So the branch and the text are
    /// quoted with Rust's escaping, which turns quotes, line breaks, and
    /// control and invisible characters into escapes: an entry reads as
    /// one quoted value and cannot pass for the note's own text or markup.
    pub fn line(&self, max: usize) -> String {
        let mut line = format!("#{} [{}", self.id, self.kind);
        if self.pinned {
            line.push_str(", pinned");
        }
        line.push(']');
        if let Some(branch) = &self.branch {
            line.push_str(&format!(" (branch {branch:?})"));
        }
        if let Some(name) = &self.name {
            line.push_str(&format!(" {name}:"));
        }
        let flat: Vec<&str> = self.text.split_whitespace().collect();
        let room = max.saturating_sub(line.chars().count() + 3);
        line.push_str(&format!(" {:?}", cut(&flat.join(" "), room)));
        if self.details.is_some() {
            line.push_str(" [+details]");
        }
        line
    }
}

/// `text` cut to at most `max` characters, ending in an ellipsis when cut.
pub(crate) fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// What [`crate::Memory::add`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    New(i64),
    /// The entry of that name was updated.
    Updated(i64),
    /// The same text was already kept in that scope; it is marked used.
    Again(i64),
}

impl Added {
    pub fn id(self) -> i64 {
        match self {
            Self::New(id) | Self::Updated(id) | Self::Again(id) => id,
        }
    }
}

/// The entries an agent should see before it starts, one line each, within
/// a character budget.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Index {
    pub lines: Vec<String>,
    /// The entries the lines show, in order.
    pub ids: Vec<i64>,
    /// How many entries the scope holds in all.
    pub total: usize,
}

impl Index {
    /// The lines, and when some did not fit, how to see the rest.
    pub fn text(&self) -> String {
        let mut text = self.lines.join("\n");
        if self.lines.len() < self.total {
            text.push_str(&format!(
                "\n(Showing {} of {}; `sheprd memory search WORDS` finds the rest.)",
                self.lines.len(),
                self.total
            ));
        }
        text
    }
}
