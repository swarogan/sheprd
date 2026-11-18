//! The command line, parsed from OS strings so paths need not be UTF-8.
//! Parsing is pure: reading token files and the state directory happens
//! after, in `main`.

use crate::authorized::{self, Entry};
use sheprd_relay::{Endpoint, wire::Key};
use std::{ffi::OsString, path::PathBuf};

pub const USAGE: &str = "\
sheprd-relay-agent: lets Sheprd reach this machine's SSH server through relays

Usage:
  sheprd-relay-agent run --relay ENDPOINT [--token-file FILE] [--relay ENDPOINT ...] [--forward PORT]
  sheprd-relay-agent status [--json]
  sheprd-relay-agent authorize DIAL_KEY TUNNEL_KEY [LABEL]
  sheprd-relay-agent revoke DIAL_KEY
  sheprd-relay-agent --help | --version

run         Listen on each relay and forward the sessions of authorized
            desktops to the SSH server on 127.0.0.1 (port 22 by default).
            --token-file applies to the --relay before it.
status      This agent's route, tunnel identity, desktops, and relays.
authorize   Allow a desktop to dial, by its base64url keys. A running agent
            picks it up within seconds.
revoke      Remove a desktop; its sessions end at once.

Every command takes --state-dir DIR (default: ~/.local/state/sheprd-relay-agent,
or ~/Library/Application Support/sheprd-relay-agent on macOS).
";

/// What was asked for, and where the state lives (`None`: the default).
#[derive(Debug, Eq, PartialEq)]
pub struct Invocation {
    pub command: Command,
    pub state_dir: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Authorize(Entry),
    Help,
    Revoke(Key),
    Run(Run),
    Status { json: bool },
    Version,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Run {
    /// The SSH server's port on loopback.
    pub forward: u16,
    pub relays: Vec<RelayArg>,
}

/// A relay as given, with the file its listen token is in, if any.
#[derive(Debug, Eq, PartialEq)]
pub struct RelayArg {
    pub endpoint: Endpoint,
    pub token_file: Option<PathBuf>,
}

/// Why the command line is not one the agent takes.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum Error {
    // keep-sorted start
    #[error("`{0}` is not a base64url dial key")]
    DialKey(String),
    #[error("`{0}` is not a relay endpoint")]
    Endpoint(String),
    #[error("`{0}` needs a value")]
    MissingValue(&'static str),
    #[error("no command; see --help")]
    NoCommand,
    #[error("`run` needs at least one --relay")]
    NoRelay,
    #[error("`{0}` is not a port")]
    Port(String),
    #[error("`--token-file` must follow a --relay")]
    TokenWithoutRelay,
    #[error("`{0}` is not a base64url tunnel key")]
    TunnelKey(String),
    #[error("`{0}` is not something this command takes")]
    Unexpected(String),
    // keep-sorted end
}

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Invocation, Error> {
    let mut args = args.into_iter();
    let mut state_dir = None;
    let mut words = Vec::new();
    let mut run = Run {
        forward: 22,
        relays: Vec::new(),
    };
    let mut json = false;
    let mut run_flag = None;
    let mut status_flag = false;
    while let Some(arg) = args.next() {
        let mut value = |name: &'static str| args.next().ok_or(Error::MissingValue(name));
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(invocation(Command::Help, None)),
            Some("--version" | "-V") => return Ok(invocation(Command::Version, None)),
            Some("--state-dir") => state_dir = Some(PathBuf::from(value("--state-dir")?)),
            Some("--json") => {
                json = true;
                status_flag = true;
            }
            Some("--relay") => {
                run_flag = Some("--relay");
                let text = value("--relay")?;
                let text = text.to_string_lossy();
                let endpoint =
                    Endpoint::parse(&text).map_err(|_| Error::Endpoint(text.to_string()))?;
                run.relays.push(RelayArg {
                    endpoint,
                    token_file: None,
                });
            }
            Some("--token-file") => {
                let file = PathBuf::from(value("--token-file")?);
                let relay = run.relays.last_mut().ok_or(Error::TokenWithoutRelay)?;
                relay.token_file = Some(file);
            }
            Some("--forward") => {
                run_flag = Some("--forward");
                let text = value("--forward")?.to_string_lossy().into_owned();
                run.forward = text.parse().map_err(|_| Error::Port(text))?;
            }
            _ => words.push(arg.to_string_lossy().into_owned()),
        }
    }
    let mut words = words.into_iter();
    let command = match words.next().as_deref() {
        None => return Err(Error::NoCommand),
        Some("run") if run.relays.is_empty() => return Err(Error::NoRelay),
        Some("run") => Command::Run(run),
        Some("status") => Command::Status { json },
        Some("authorize") => {
            let dial = words.next().ok_or(Error::MissingValue("DIAL_KEY"))?;
            let tunnel = words.next().ok_or(Error::MissingValue("TUNNEL_KEY"))?;
            let label = words.by_ref().collect::<Vec<_>>().join(" ");
            authorized::dial_key(&dial).ok_or_else(|| Error::DialKey(dial.clone()))?;
            let entry = Entry::parse(&dial, &tunnel, &label).ok_or(Error::TunnelKey(tunnel))?;
            Command::Authorize(entry)
        }
        Some("revoke") => {
            let dial = words.next().ok_or(Error::MissingValue("DIAL_KEY"))?;
            Command::Revoke(authorized::dial_key(&dial).ok_or(Error::DialKey(dial))?)
        }
        Some(other) => return Err(Error::Unexpected(other.to_owned())),
    };
    if let Some(extra) = words.next() {
        return Err(Error::Unexpected(extra));
    }
    // Flags of another command are refused rather than ignored.
    if let (Some(flag), false) = (run_flag, matches!(command, Command::Run(_))) {
        return Err(Error::Unexpected(flag.to_owned()));
    }
    if status_flag && !matches!(command, Command::Status { .. }) {
        return Err(Error::Unexpected("--json".to_owned()));
    }
    Ok(invocation(command, state_dir))
}

fn invocation(command: Command, state_dir: Option<PathBuf>) -> Invocation {
    Invocation { command, state_dir }
}

#[cfg(test)]
mod tests;
