//! A smarter line for a chat's row: Claude, run once on its own, reads the
//! end of the chat's transcript and says where it stands.
//! The chat's session, history, and context are untouched, as nothing is
//! sent to it; the summary is the daemon's and lives with the chat's link.

mod error;

pub(crate) use error::Error;

use super::chat::excerpt;
use serde::{Deserialize, Serialize};
use sheprd_chat_protocol::{EntryKind, Transcript};
use std::{
    ffi::OsString,
    io::{Read, Write},
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// How long the agent may take to answer before it is stopped.
pub(crate) const WAIT: Duration = Duration::from_secs(90);
/// The most of the transcript the agent reads, in characters, newest kept.
const MAX_INPUT_CHARS: usize = 12_000;
/// The most of one message the agent reads, in characters.
const MAX_ENTRY_CHARS: usize = 2_000;
/// The most of the agent's answer read, in bytes; the rest is dropped.
const MAX_OUTPUT_BYTES: u64 = 16 * 1024;
/// The longest summary a row keeps, in characters.
const MAX_SUMMARY_CHARS: usize = 280;
/// Summaries running at once across every chat; one more is refused.
const MAX_RUNNING: usize = 3;
/// How often a running agent is checked for having exited.
const POLL: Duration = Duration::from_millis(100);
/// How long the agent's output may take to close once it has exited.
const DRAIN: Duration = Duration::from_secs(2);

/// What the agent is asked; the transcript follows on its input.
const PROMPT: &str = "The input is the end of a conversation between a person and a coding \
agent. Summarize where it stands for one row of a list on a phone: at most two short \
sentences, plain text, no Markdown, no quotes, no preamble. Say what is being worked on and \
what is done, next, or blocked.";

/// A chat's summary as its row shows it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum RowSummary {
    /// The agent is reading the chat.
    Pending {
        since_ms: u64,
    },
    Ready {
        text: String,
        at_ms: u64,
    },
    /// Why there is none, as the person reads it.
    Failed {
        reason: String,
        at_ms: u64,
    },
}

/// Claude's arguments for one answer to `prompt`: no tools, no saved
/// session, only the user's own settings, and no MCP server. Every chat is
/// answered by Claude, whatever its agent: Codex and Gemini cannot be run
/// without tools or MCP servers that reach past the answer.
fn claude_args(prompt: &str) -> Vec<OsString> {
    [
        "-p",
        "--tools",
        "",
        "--no-session-persistence",
        "--setting-sources",
        "user",
        "--strict-mcp-config",
        "--output-format",
        "text",
        prompt,
    ]
    .iter()
    .map(OsString::from)
    .collect()
}

/// The person's prompts and the agent's answers, oldest first, as much of
/// the end as fits; empty when nothing was said yet.
pub(crate) fn excerpt_of(transcript: &Transcript) -> String {
    let mut parts = Vec::new();
    let mut left = MAX_INPUT_CHARS;
    for entry in transcript.entries.iter().rev() {
        let (who, text) = match &entry.kind {
            EntryKind::User { text, .. } => ("Person", text),
            EntryKind::Agent { text } => ("Agent", text),
            _ => continue,
        };
        let line = format!("{who}: {}", excerpt(text, MAX_ENTRY_CHARS));
        let size = line.chars().count();
        if size > left {
            break;
        }
        left -= size;
        parts.push(line);
    }
    parts.reverse();
    parts.join("\n\n")
}

/// Counts a running summary for [`MAX_RUNNING`] until dropped.
pub(crate) struct Slot;

static RUNNING: AtomicUsize = AtomicUsize::new(0);

impl Slot {
    pub(crate) fn take() -> Result<Self, Error> {
        RUNNING
            .try_update(Ordering::AcqRel, Ordering::Acquire, |running| {
                (running < MAX_RUNNING).then_some(running + 1)
            })
            .map(|_| Self)
            .map_err(|_| Error::Busy)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Blocking: the summary of `input`.
pub(crate) fn run(input: &str) -> Result<String, Error> {
    ask(PROMPT, input)
}

/// Blocking: Claude's answer to `prompt` about `input`, cut to
/// [`MAX_SUMMARY_CHARS`].
pub(crate) fn ask(prompt: &str, input: &str) -> Result<String, Error> {
    run_first([("claude".into(), claude_args(prompt))], input, WAIT)
}

/// The answer of the first of `candidates` that gives one, each run for at
/// most `wait`. One not installed, failing, or answering nothing (an agent
/// signed out, say) passes to the next; one that overruns is killed and
/// reaped here, and ends the search, so a summary never waits for two.
pub(crate) fn run_first(
    candidates: impl IntoIterator<Item = (OsString, Vec<OsString>)>,
    input: &str,
    wait: Duration,
) -> Result<String, Error> {
    let mut last = Error::NoAgent;
    for (program, args) in candidates {
        match run_one(program, args, input, wait) {
            Err(error @ (Error::NoAgent | Error::Failed | Error::Empty)) => last = error,
            answer => return answer,
        }
    }
    Err(last)
}

/// `program`'s answer, or [`Error::NoAgent`] when it is not installed.
fn run_one(
    program: OsString,
    args: Vec<OsString>,
    input: &str,
    wait: Duration,
) -> Result<String, Error> {
    // A fresh folder only this user can enter, removed after the run: a
    // shared one such as Linux's `/tmp` could hold another user's agent
    // settings, hooks, or MCP servers, which the agent would load.
    let folder = tempfile::Builder::new()
        .prefix("sheprd-agent-")
        .tempdir()
        .map_err(Error::Start)?;
    {
        let mut command = sheprd_client::new_command(&program);
        // A user service's `PATH` is bare; the login shell's finds the
        // agents the user's own shell runs.
        sheprd_client::login_env::apply(&mut command);
        // Its own process group, so what it starts ends with it.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        // Away from any checkout, so the agent reads only what it is given.
        command
            .args(args)
            .current_dir(folder.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match command.spawn() {
            Ok(child) => answer(child, input.to_owned(), wait),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Error::NoAgent),
            Err(error) => Err(Error::Start(error)),
        }
    }
}

fn answer(mut child: std::process::Child, input: String, wait: Duration) -> Result<String, Error> {
    if let Some(mut stdin) = child.stdin.take() {
        // A failed write shows as the agent's own failure or empty answer.
        thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    let (sender, output) = mpsc::sync_channel(1);
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut limited = stdout.take(MAX_OUTPUT_BYTES);
            let _ = limited.read_to_end(&mut bytes);
            // The rest is drained so the agent never blocks on a full pipe.
            let _ = std::io::copy(&mut limited.into_inner(), &mut std::io::sink());
            let _ = sender.send(bytes);
        });
    }
    let deadline = Instant::now() + wait;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            Ok(None) | Err(_) => {
                end_group(&child);
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::TimedOut);
            }
        }
    };
    // What it started and left running would hold the output open, and the
    // run's slot with it.
    end_group(&child);
    let bytes = output.recv_timeout(DRAIN).unwrap_or_default();
    if !status.success() {
        return Err(Error::Failed);
    }
    let text = excerpt(&String::from_utf8_lossy(&bytes), MAX_SUMMARY_CHARS);
    if text.is_empty() {
        return Err(Error::Empty);
    }
    Ok(text)
}

/// Kills what is left of the agent's process group. Windows has none here;
/// the bounded wait for the output keeps a leftover child from holding
/// the run there.
fn end_group(child: &std::process::Child) {
    #[cfg(unix)]
    if let Some(group) = i32::try_from(child.id())
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    #[cfg(not(unix))]
    let _ = child;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
