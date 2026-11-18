//! The texts a note is made of: the built-in templates in this crate's
//! `prompts` folder, each replaced by a copy the user edits in
//! `chat-prompts` under Herdr's configuration directory, and the user's own
//! instructions. Read once when the chat starts, on the machine the agent
//! runs on.

use std::{
    borrow::Cow,
    io,
    path::{Path, PathBuf},
};

use sheprd_chat_protocol::{Device, Origin};

use super::template;

/// The longest file read, in bytes; a longer one is not used.
pub(crate) const MAX_FILE: u64 = 16 * 1024;

/// The folder the user's copies live in, under the configuration root.
const DIR: &str = "herdr/chat-prompts";
/// Where instructions lived before the folder; read when it has none.
const LEGACY_INSTRUCTIONS: &str = "herdr/chat-system-prompt.md";
const INSTRUCTIONS: &str = "instructions.md";

/// Pictures read per message and their size, as [`crate::pictures`] and
/// [`crate::files`] bound them, for the templates to quote.
const MAX_PICTURES: usize = crate::pictures::MAX_PICTURES;
const MAX_IMAGE_MB: u64 = crate::files::MAX_IMAGE / (1024 * 1024);

/// Declares each template once: its kind, its file, and its built-in text.
macro_rules! kinds {
    ($($kind:ident => $file:literal,)*) => {
        /// Which template a part of the note comes from.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(crate) enum Kind {
            $($kind,)*
        }

        impl Kind {
            const ALL: &[Self] = &[$(Self::$kind,)*];

            fn file(self) -> &'static str {
                match self {
                    $(Self::$kind => $file,)*
                }
            }

            fn builtin(self) -> &'static str {
                match self {
                    $(Self::$kind => include_str!(concat!("../../prompts/", $file)),)*
                }
            }
        }
    };
}

kinds! {
    // keep-sorted start
    Desktop => "desktop.md",
    Intro => "intro.md",
    Phone => "phone.md",
    Remote => "remote.md",
    Terminal => "terminal.md",
    // keep-sorted end
}

impl Kind {
    /// The template for a person typing at `origin`.
    pub(crate) fn of(origin: Origin) -> Self {
        match origin.device {
            Device::PhoneApp | Device::PhoneWeb => Self::Phone,
            Device::Desktop if origin.opens_paths() => Self::Desktop,
            Device::Terminal if origin.opens_paths() => Self::Terminal,
            Device::Desktop | Device::Terminal | Device::Other => Self::Remote,
        }
    }
}

/// Why a file of the user's was not used, or not all of it.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Problem {
    #[error("{} is not used: it is larger than 16 KiB", path.display())]
    TooLarge { path: PathBuf },
    #[error("{} is not used: it is not UTF-8 text", path.display())]
    NotText { path: PathBuf },
    #[error("{} is not used: it cannot be read", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} uses {{{{ {name} }}}}, which Sheprd does not know; it is sent as written", path.display())]
    UnknownName { name: String, path: PathBuf },
}

/// The templates and instructions a chat uses.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Prompts {
    /// The user's instructions for every chat, as written: they are filled
    /// in when they go, so they name the agent running then.
    instructions: Option<String>,
    /// The user's copies, by kind; a kind without one uses its built-in.
    templates: Vec<(Kind, String)>,
}

/// What every template may name besides where the person is.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Facts {
    pub(crate) agent: String,
    pub(crate) cwd: String,
}

/// Names that depend on where the person is, which instructions may not
/// use: instructions go once, wherever the person is then.
const PLACE_NAMES: &[&str] = &["connection", "device", "where"];
const FACT_NAMES: &[&str] = &["agent", "cwd", "max_image_mb", "max_pictures", "os"];

/// What the place names of a template stand for.
#[derive(Clone, Copy, Debug)]
enum Where {
    /// The person is at this origin.
    At(Origin),
    /// No client has said where the person is.
    Unknown,
    /// The text may not name the place, which stays as written.
    Unnamed,
}

/// A file of the user's, as far as using it goes.
enum Found {
    Missing,
    /// Empty, or not usable, which says why.
    Unused,
    Text(String),
}

impl Prompts {
    /// Built-in templates with `instructions`.
    #[cfg(test)]
    pub(crate) fn with_instructions(instructions: &str) -> Self {
        Self {
            instructions: Some(instructions.to_owned()),
            templates: Vec::new(),
        }
    }

    /// The user's folder on this machine.
    pub(crate) fn dir() -> Option<PathBuf> {
        let root = sheprd_client::local_path::config_root().ok()?;
        Some(root.join(DIR))
    }

    /// Reads the user's files from `dir`, and what was wrong with any of
    /// them. Only a missing `instructions.md` falls back to the older file
    /// beside the folder: an empty one clears the instructions.
    pub(crate) fn load(dir: &Path) -> (Self, Vec<Problem>) {
        let mut problems = Vec::new();
        let mut templates = Vec::new();
        for &kind in Kind::ALL {
            let path = dir.join(kind.file());
            if let Found::Text(text) = read(&path, &mut problems) {
                check(&path, &text, &[PLACE_NAMES, FACT_NAMES], &mut problems);
                templates.push((kind, text));
            }
        }
        let mut path = dir.join(INSTRUCTIONS);
        let mut found = read(&path, &mut problems);
        if let Found::Missing = found
            && let Some(root) = dir.parent().and_then(Path::parent)
        {
            path = root.join(LEGACY_INSTRUCTIONS);
            found = read(&path, &mut problems);
        }
        let instructions = match found {
            Found::Text(text) => {
                check(&path, &text, &[FACT_NAMES], &mut problems);
                Some(text)
            }
            Found::Missing | Found::Unused => None,
        };
        let prompts = Self {
            instructions,
            templates,
        };
        (prompts, problems)
    }

    /// The instructions filled in with `facts`.
    pub(crate) fn instructions(&self, facts: &Facts) -> Option<String> {
        let text = self.instructions.as_deref()?;
        Some(fill(text, facts, Where::Unnamed))
    }

    /// The template of `kind` filled in for a person at `origin`, or at an
    /// unknown place when no client said.
    pub(crate) fn render(&self, kind: Kind, facts: &Facts, origin: Option<Origin>) -> String {
        let template = self
            .templates
            .iter()
            .find(|(known, _)| *known == kind)
            .map_or(kind.builtin(), |(_, text)| text.as_str());
        fill(template, facts, origin.map_or(Where::Unknown, Where::At))
    }
}

fn fill(template: &str, facts: &Facts, at: Where) -> String {
    let place = |name: &str| match at {
        Where::At(origin) => Some(match name {
            "connection" => Cow::Borrowed(super::connection_name(origin.via)),
            "device" => Cow::Borrowed(super::device_name(origin.device)),
            _ => Cow::Owned(super::place_phrase(origin)),
        }),
        Where::Unknown => Some(Cow::Borrowed(match name {
            "connection" => "an unknown connection",
            "device" => "a Sheprd client",
            _ => "in a Sheprd client",
        })),
        Where::Unnamed => None,
    };
    let rendered = template::render(template, |name| {
        Some(match name {
            "agent" => Cow::Borrowed(facts.agent.as_str()),
            "cwd" => Cow::Borrowed(facts.cwd.as_str()),
            "max_image_mb" => Cow::Owned(MAX_IMAGE_MB.to_string()),
            "max_pictures" => Cow::Owned(MAX_PICTURES.to_string()),
            "os" => Cow::Borrowed(std::env::consts::OS),
            name if PLACE_NAMES.contains(&name) => return place(name),
            _ => return None,
        })
    });
    rendered.trim().to_owned()
}

/// The file at `path`: missing, empty or unusable (a problem says why), or
/// its text. The read itself stops past [`MAX_FILE`], so a file that grows
/// meanwhile is never read whole.
fn read(path: &Path, problems: &mut Vec<Problem>) -> Found {
    use std::io::Read as _;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Found::Missing,
        Err(source) => {
            problems.push(Problem::Unreadable {
                path: path.to_owned(),
                source,
            });
            return Found::Unused;
        }
    };
    let mut bytes = Vec::new();
    if let Err(source) = file.take(MAX_FILE + 1).read_to_end(&mut bytes) {
        problems.push(Problem::Unreadable {
            path: path.to_owned(),
            source,
        });
        return Found::Unused;
    }
    if bytes.len() as u64 > MAX_FILE {
        problems.push(Problem::TooLarge {
            path: path.to_owned(),
        });
        return Found::Unused;
    }
    let Ok(text) = String::from_utf8(bytes) else {
        problems.push(Problem::NotText {
            path: path.to_owned(),
        });
        return Found::Unused;
    };
    let text = text.trim();
    if text.is_empty() {
        Found::Unused
    } else {
        Found::Text(text.to_owned())
    }
}

/// Warns once about each name in `text` none of `known` holds.
fn check(path: &Path, text: &str, known: &[&[&str]], problems: &mut Vec<Problem>) {
    let mut warned: Vec<&str> = Vec::new();
    for name in template::names(text) {
        if known.iter().any(|names| names.contains(&name)) || warned.contains(&name) {
            continue;
        }
        warned.push(name);
        problems.push(Problem::UnknownName {
            name: name.to_owned(),
            path: path.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests;
