//! What a herdr-core terminal runs in front of its shell, for a phone's row.

use crate::hub::Job;
use sheprd_client::herdr::processes::program;
use sheprd_core_client::wire::Terminal as Record;

/// The most characters of a command line a row carries; a longer one ends
/// in an ellipsis, as a phone shows one line of it anyway.
const MAX_COMMAND_CHARS: usize = 120;

/// The programs a terminal is left at when nothing else runs in it.
const SHELLS: &[&str] = &[
    // keep-sorted start
    "bash",
    "cmd.exe",
    "csh",
    "dash",
    "elvish",
    "fish",
    "ksh",
    "nu",
    "powershell",
    "powershell.exe",
    "pwsh",
    "pwsh.exe",
    "sh",
    "tcsh",
    "xonsh",
    "zsh",
    // keep-sorted end
];

/// What `record` runs in front, or `None` when the core does not say.
///
/// The core reports its foreground job's leader. While that is the
/// terminal's own program, a shell waits at its prompt; a terminal started
/// with some other program runs that program.
pub(super) fn of(record: &Record) -> Option<Job> {
    let foreground = record.foreground.as_ref()?;
    let own = record.pid.is_some_and(|pid| pid == foreground.pid);
    let argv: &[String] = if foreground.argv.is_empty() {
        std::slice::from_ref(&foreground.name)
    } else {
        &foreground.argv
    };
    let name = program(argv)
        .map(|name| name.trim_start_matches('-'))
        .filter(|name| !name.is_empty());
    if own && name.is_some_and(|name| SHELLS.contains(&name)) {
        return Some(Job::Idle);
    }
    let command = command(name?, argv.get(1..).unwrap_or_default());
    Some(Job::Running { command })
}

/// `name` and its arguments on one bounded line.
fn command(name: &str, args: &[String]) -> String {
    let words = std::iter::once(name).chain(args.iter().map(String::as_str));
    let mut line = String::new();
    for word in words.filter(|word| !word.is_empty()) {
        if !line.is_empty() {
            line.push(' ');
        }
        line.extend(word.chars().map(|c| if c.is_control() { ' ' } else { c }));
        if line.chars().count() > MAX_COMMAND_CHARS {
            break;
        }
    }
    if line.chars().count() > MAX_COMMAND_CHARS {
        line = line.chars().take(MAX_COMMAND_CHARS - 1).collect();
        line.push('…');
    }
    line
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
