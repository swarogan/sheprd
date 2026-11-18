//! `memory`, run in this process for the repository of the current
//! directory, so Sheprd need not run: the memory file is keyed by the Git
//! common directory, which every worktree of the repository shares, and
//! lives on the host the repository is on.
use super::super::{Context, Output};
use crate::{
    cli::control::{MemoryAdd, MemoryCommand},
    control::{Error as ControlError, ErrorCode},
};
use sheprd_memory::{Added, Checkout, Entry, Error, Memory, New, Scope};
use std::fmt::Write as _;

/// The longest line a listing prints for one entry, in characters.
const LINE: usize = 200;

fn rejected(code: ErrorCode, message: impl Into<String>) -> crate::Error {
    ControlError::Rejected {
        code,
        message: message.into(),
    }
    .into()
}

/// The display boundary: the message, and the exit status it deserves.
fn failed(error: Error) -> crate::Error {
    let code = match error {
        Error::Empty
        | Error::Kind(_)
        | Error::Name { .. }
        | Error::NotInCheckout
        | Error::SameText { .. }
        | Error::TooLong { .. }
        | Error::TooManyPinned { .. }
        | Error::Unsafe(_) => ErrorCode::InvalidRequest,
        _ => ErrorCode::Failed,
    };
    rejected(code, error.to_string())
}

/// The current directory's memory, and the branch checked out there.
fn here() -> crate::Result<(Memory, Option<String>)> {
    let dir = std::env::current_dir().map_err(crate::Error::LocalGit)?;
    let checkout = Checkout::find(&dir).map_err(failed)?;
    let path =
        sheprd_memory::path(&checkout.common_dir).ok_or_else(|| failed(Error::NoStateDirectory))?;
    Ok((Memory::open(&path).map_err(failed)?, checkout.branch()))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

/// One line per entry.
pub(super) fn describe(entries: &[Entry]) -> String {
    let mut text = String::new();
    for entry in entries {
        let _ = writeln!(text, "{}", entry.line(LINE));
    }
    text.trim_end().to_owned()
}

/// An entry in full: its line, then its text and details unwrapped.
pub(super) fn describe_one(entry: &Entry) -> String {
    let mut text = format!("{}\n\n{}", entry.line(LINE), entry.text);
    if let Some(details) = &entry.details {
        let _ = write!(text, "\n\n{details}");
    }
    text
}

fn entries(entries: Vec<Entry>, empty: &str) -> crate::Result<Output> {
    let json = serde_json::json!({ "entries": entries });
    if entries.is_empty() {
        return Ok(Output::data(empty, &json)?.nothing());
    }
    Output::data(describe(&entries), &json)
}

fn scope(all: bool, branch: Option<&str>) -> Scope<'_> {
    if all {
        Scope::All
    } else {
        Scope::Here { branch }
    }
}

fn missing(id: i64) -> crate::Error {
    rejected(
        ErrorCode::NotFound,
        format!("This repository's memory has no #{id}"),
    )
}

fn add(memory: &mut Memory, branch: Option<&str>, add: &MemoryAdd) -> crate::Result<Output> {
    let target = match (add.branch, branch) {
        (false, _) => None,
        (true, Some(branch)) => Some(branch),
        (true, None) => {
            return Err(rejected(
                ErrorCode::InvalidRequest,
                "--branch needs a checked-out branch, and HEAD is detached",
            ));
        }
    };
    let new = New {
        branch: target,
        name: add.name.as_deref(),
        kind: add.kind,
        text: &add.text,
        details: add.details.as_deref(),
        pinned: add.pinned,
    };
    let added = memory.add(&new, now()).map_err(failed)?;
    let place = target.map_or_else(|| "the repository".to_owned(), str::to_owned);
    let message = match added {
        Added::New(id) => format!("Remembered #{id} for {place}"),
        Added::Updated(id) => format!("Updated #{id} for {place}"),
        Added::Again(id) => format!("Already remembered as #{id} for {place}"),
    };
    let state = match added {
        Added::New(_) => "new",
        Added::Updated(_) => "updated",
        Added::Again(_) => "again",
    };
    Output::data(
        message,
        &serde_json::json!({ "id": added.id(), "added": state }),
    )
}

pub(in crate::control::client) fn memory(
    command: MemoryCommand,
    _: &Context,
) -> crate::Result<Output> {
    let (mut memory, branch) = here()?;
    let branch = branch.as_deref();
    match command {
        MemoryCommand::List { all, limit } => entries(
            memory.list(scope(all, branch), limit).map_err(failed)?,
            "This repository has no memory yet",
        ),
        MemoryCommand::Search { query, all, limit } => entries(
            memory
                .search(scope(all, branch), &query, limit, Some(now()))
                .map_err(failed)?,
            "Nothing in memory matches",
        ),
        MemoryCommand::Index { chars } => {
            let index = memory.index(branch, chars).map_err(failed)?;
            let output = Output::data(index.text(), &index)?;
            Ok(if index.total == 0 {
                Output::data("This repository has no memory yet", &index)?.nothing()
            } else {
                output
            })
        }
        MemoryCommand::Get { id } => {
            let entry = memory
                .get(id, now())
                .map_err(failed)?
                .ok_or_else(|| missing(id))?;
            Output::data(describe_one(&entry), &entry)
        }
        MemoryCommand::Add(command) => add(&mut memory, branch, &command),
        MemoryCommand::Pin { id, pinned } => {
            if !memory.pin(id, pinned).map_err(failed)? {
                return Err(missing(id));
            }
            let verb = if pinned { "Pinned" } else { "Unpinned" };
            Ok(Output::message(format!("{verb} #{id}")))
        }
        MemoryCommand::Forget { id } => {
            if !memory.forget(id).map_err(failed)? {
                return Err(missing(id));
            }
            Ok(Output::message(format!("Forgot #{id}")))
        }
    }
}
