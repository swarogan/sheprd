//! Whether macOS's application firewall will keep letting phones reach the
//! daemon. The firewall remembers "Allow incoming connections" for a binary
//! signed with a developer ID by its signer, so the approval survives
//! updates. For an ad-hoc or unsigned binary, such as a local build, it
//! remembers that exact build: once the file changes, the next start of the
//! daemon (a restart of the Mac, of the service, or launchd's own) is
//! blocked until someone clicks Allow at the computer. Loopback still works,
//! so nothing on the desktop shows it.
//!
//! The answer is advisory: a check that fails, runs past its deadline, or
//! prints what this module does not recognise warns of nothing. Its
//! failures stay here, for the caller's log; nothing shows them to people.
//! Everything here blocks, for at most [`DEADLINE`] per command.

mod error;

pub use error::{Error, Result};
use std::{ffi::OsStr, path::Path, process::ExitStatus, time::Duration};

/// How long each command may take before the check gives up on it.
pub const DEADLINE: Duration = Duration::from_secs(5);
/// How much of a command's output is read; both answers fit in a fraction.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const OUTPUT_LIMIT: u64 = 64 * 1024;

/// How long an approval of the daemon lasts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Approval {
    /// No firewall to approve it, a signer the firewall remembers, or an
    /// answer this module could not read.
    #[default]
    Lasting,
    /// The firewall is on and remembers only this build of the daemon.
    ThisBuild,
}

/// Who signed a binary, as `codesign -dv` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
enum Signer {
    /// A team, as a developer ID or development certificate names.
    Team,
    /// Ad-hoc ("TeamIdentifier=not set") or not signed at all.
    Nobody,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const SOCKETFILTERFW: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const CODESIGN: &str = "/usr/bin/codesign";

/// How long the firewall's approval of `program` lasts. Runs
/// `socketfilterfw` and `codesign`, neither of which needs privileges; off
/// macOS there is no such firewall.
pub fn approval(program: &Path) -> Result<Approval> {
    #[cfg(target_os = "macos")]
    {
        let (status, state) = output(SOCKETFILTERFW, ["--getglobalstate".as_ref()], DEADLINE)?;
        if !status.success() || !firewall_on(&state) {
            return Ok(Approval::Lasting);
        }
        let (status, signature) =
            output(CODESIGN, ["-dv".as_ref(), program.as_os_str()], DEADLINE)?;
        Ok(match signer(status.success(), &signature) {
            Some(Signer::Nobody) => Approval::ThisBuild,
            Some(Signer::Team) | None => Approval::Lasting,
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = program;
        Ok(Approval::Lasting)
    }
}

/// `program`'s exit status and both its streams, as one text: `codesign
/// -dv` writes to stderr, and fails on an unsigned file. Gives up after
/// `deadline`, killing and reaping the child.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn output<'a>(
    program: &'static str,
    args: impl IntoIterator<Item = &'a OsStr>,
    deadline: Duration,
) -> Result<(ExitStatus, String)> {
    use std::io::Read;
    let (reader, writer) = std::io::pipe().map_err(|source| Error::Spawn { program, source })?;
    let mut child = {
        // The command holds the pipe's write ends until it is dropped, and
        // the reader below sees the end of the output only once none is left.
        let stdout = writer
            .try_clone()
            .map_err(|source| Error::Spawn { program, source })?;
        sheprd_client::new_command(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(stdout)
            .stderr(writer)
            .spawn()
            .map_err(|source| Error::Spawn { program, source })?
    };
    // One deadline for the whole command: reading its output, then its exit.
    let ends = std::time::Instant::now() + deadline;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut text = Vec::new();
        let read = reader
            .take(OUTPUT_LIMIT)
            .read_to_end(&mut text)
            .map(|_| text);
        let _ = send.send(read);
    });
    let text = match receive.recv_timeout(deadline) {
        Ok(Ok(text)) => text,
        Ok(Err(source)) => {
            reap(&mut child);
            return Err(Error::Read { program, source });
        }
        Err(_) => {
            reap(&mut child);
            return Err(Error::Timeout { program });
        }
    };
    // The output ended or reached its limit. A child that closed its
    // streams, or is stuck writing past the limit, may still not exit: it
    // gets what is left of the deadline, then is killed like any other.
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < ends => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                reap(&mut child);
                return Err(Error::Timeout { program });
            }
            Err(source) => {
                reap(&mut child);
                return Err(Error::Read { program, source });
            }
        }
    };
    Ok((status, String::from_utf8_lossy(&text).into_owned()))
}

/// Ends a child the check gave up on, so it leaves no zombie.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn reap(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Whether `socketfilterfw --getglobalstate` says the firewall is on:
/// "Firewall is enabled. (State = 1)", or 2 when it blocks all but
/// essential services. Anything else, including words it never printed,
/// is taken as off, so a change in its wording never raises a warning.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn firewall_on(text: &str) -> bool {
    text.split_once("(State = ")
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(state, _)| state.trim().parse::<u8>().ok())
        .is_some_and(|state| state > 0)
}

/// Who signed the binary `codesign -dv` described, or `None` when its
/// answer is not one of those this module knows: a failure other than an
/// unsigned file (unreadable, missing, not code), or wording it never used.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn signer(success: bool, text: &str) -> Option<Signer> {
    if text.contains("code object is not signed at all") {
        return Some(Signer::Nobody);
    }
    if !success {
        return None;
    }
    let team = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("TeamIdentifier="))?;
    match team.trim() {
        "" => None,
        "not set" => Some(Signer::Nobody),
        _ => Some(Signer::Team),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
