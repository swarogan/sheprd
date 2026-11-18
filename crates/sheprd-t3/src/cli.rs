//! `sheprd-t3`'s arguments, parsed without touching a server.

use std::{ffi::OsString, path::PathBuf};

pub const HELP: &str = "\
sheprd-t3: read T3 Code threads as Sheprd chats

Usage:
  sheprd-t3 pair --token-file FILE [--server URL]
  sheprd-t3 threads --token-file FILE [--server URL] [--json]
  sheprd-t3 show --token-file FILE [--server URL] [--json] THREAD

Commands:
  pair      Sign in with a one-time code from `t3 auth pairing create` on
            the server's machine, read from stdin, and keep the token in
            FILE (mode 0600), usable only with that server
  threads   List the server's threads
  show      Print one thread as a chat transcript

Options:
  --server URL       The server's address; by default the T3 Code server
                     running on this computer ($T3CODE_HOME or ~/.t3)
  --token-file FILE  Where the access token is kept
  --json             Print JSON
  -h, --help         Show this help
";

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Help,
    Pair,
    Threads,
    Show { thread: String },
}

#[derive(Debug, Eq, PartialEq)]
pub struct Options {
    pub command: Command,
    pub json: bool,
    pub server: Option<String>,
    pub token_file: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("{0} needs a value")]
    Missing(&'static str),
    #[error("unknown option {0}")]
    Unknown(String),
    #[error("arguments must be valid UTF-8, except file names")]
    Utf8,
    #[error("no command; see --help")]
    NoCommand,
    #[error("{0} takes one argument")]
    Arguments(&'static str),
    #[error("--token-file is required")]
    TokenFile,
}

/// `text` from the server, safe for a terminal: control characters, which
/// could move the cursor or start an escape sequence, become `\u{fffd}`.
pub fn plain(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect()
}

/// Parses the arguments after the program's name.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Options, Error> {
    let mut args = args.into_iter();
    let mut command = None;
    let mut positional = Vec::new();
    let mut json = false;
    let mut server = None;
    let mut token_file = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => {
                return Ok(Options {
                    command: Command::Help,
                    json,
                    server,
                    token_file,
                });
            }
            Some("--json") => json = true,
            Some("--server") => {
                let value = args.next().ok_or(Error::Missing("--server"))?;
                server = Some(value.into_string().map_err(|_| Error::Utf8)?);
            }
            Some("--token-file") => {
                token_file = Some(PathBuf::from(
                    args.next().ok_or(Error::Missing("--token-file"))?,
                ));
            }
            Some(option) if option.starts_with('-') => {
                return Err(Error::Unknown(option.to_owned()));
            }
            Some(word) if command.is_none() => command = Some(word.to_owned()),
            Some(word) => positional.push(word.to_owned()),
            None => return Err(Error::Utf8),
        }
    }
    let one = |name: &'static str, mut values: Vec<String>| {
        (values.len() == 1)
            .then(|| values.remove(0))
            .ok_or(Error::Arguments(name))
    };
    let command = match command.as_deref() {
        None => return Err(Error::NoCommand),
        // The code comes on stdin: arguments are visible to other users.
        Some("pair") if positional.is_empty() => Command::Pair,
        Some("pair") => return Err(Error::Arguments("pair")),
        Some("threads") if positional.is_empty() => Command::Threads,
        Some("threads") => return Err(Error::Arguments("threads")),
        Some("show") => Command::Show {
            thread: one("show", positional)?,
        },
        Some(other) => return Err(Error::Unknown(other.to_owned())),
    };
    if token_file.is_none() {
        return Err(Error::TokenFile);
    }
    Ok(Options {
        command,
        json,
        server,
        token_file,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
