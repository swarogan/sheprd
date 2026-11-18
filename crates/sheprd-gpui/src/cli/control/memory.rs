//! `memory`: what agents learned about a repository, for the next agent.
use super::{Args, CliError, unknown};
use sheprd_memory::{Kind, MAX_DETAILS, MAX_LIMIT, MAX_TEXT};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryCommand {
    /// The repository's entries and the current branch's, or every entry.
    List {
        all: bool,
        limit: usize,
    },
    /// Entries matching `query`, best first.
    Search {
        query: String,
        all: bool,
        limit: usize,
    },
    /// What an agent should see before it starts, within `chars`.
    Index {
        chars: usize,
    },
    /// One entry with its details.
    Get {
        id: i64,
    },
    /// Keep an entry for the repository, or for the current branch only.
    Add(MemoryAdd),
    Pin {
        id: i64,
        pinned: bool,
    },
    Forget {
        id: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryAdd {
    pub branch: bool,
    pub name: Option<String>,
    pub kind: Kind,
    pub text: String,
    pub details: Option<String>,
    pub pinned: bool,
}

const DEFAULT_LIMIT: usize = 20;
/// The index's default budget, about what a chat's first note carries.
const DEFAULT_INDEX_CHARS: usize = 3000;

fn id(args: &mut Args) -> Result<i64, CliError> {
    args.required_text("ID")?
        .trim_start_matches('#')
        .parse()
        .map_err(|_| CliError::InvalidValue("ID"))
}

fn add(args: &mut Args) -> Result<MemoryAdd, CliError> {
    let branch = args.flag("--branch");
    let pinned = args.flag("--pin");
    let name = args.text_option("--name")?;
    let details = args.text_option("--details")?;
    if details
        .as_ref()
        .is_some_and(|details| details.len() > MAX_DETAILS)
    {
        return Err(CliError::InvalidValue("--details over 16 KiB"));
    }
    let kind = match args.text_option("--kind")? {
        Some(kind) => kind.parse().map_err(|_| CliError::InvalidValue("--kind"))?,
        None => Kind::default(),
    };
    let text = args.words()?.ok_or(CliError::MissingArgument {
        command: "memory add",
        what: "TEXT",
    })?;
    if text.trim().len() > MAX_TEXT {
        return Err(CliError::InvalidValue(
            "TEXT over 500 bytes; put the rest in --details",
        ));
    }
    Ok(MemoryAdd {
        branch,
        name,
        kind,
        text,
        details,
        pinned,
    })
}

pub(super) fn memory(args: &mut Args) -> Result<MemoryCommand, CliError> {
    // Read before the verb, so `memory --all` lists as `memory list --all`
    // does rather than taking the flag for a verb.
    let all = args.flag("--all");
    let given = args.number("--limit", 1..=MAX_LIMIT)?;
    let limit = given.unwrap_or(DEFAULT_LIMIT);
    // The other verbs refuse them rather than ignore them.
    let listing = || {
        if all || given.is_some() {
            return Err(CliError::InvalidValue(
                "--all and --limit go with memory list and memory search",
            ));
        }
        Ok(())
    };
    let Some(verb) = args.verb() else {
        return Ok(MemoryCommand::List { all, limit });
    };
    Ok(match verb.to_str() {
        Some("list") => MemoryCommand::List { all, limit },
        Some("search") => MemoryCommand::Search {
            query: args.words()?.ok_or(CliError::MissingArgument {
                command: "memory search",
                what: "WORDS",
            })?,
            all,
            limit,
        },
        Some("index") => {
            listing()?;
            MemoryCommand::Index {
                chars: args
                    .number("--chars", 200..=16_000)?
                    .unwrap_or(DEFAULT_INDEX_CHARS),
            }
        }
        Some("get") => {
            listing()?;
            MemoryCommand::Get { id: id(args)? }
        }
        Some("add") => {
            listing()?;
            MemoryCommand::Add(add(args)?)
        }
        Some(verb @ ("pin" | "unpin")) => {
            listing()?;
            MemoryCommand::Pin {
                id: id(args)?,
                pinned: verb == "pin",
            }
        }
        Some("forget") => {
            listing()?;
            MemoryCommand::Forget { id: id(args)? }
        }
        _ => return Err(unknown("memory", verb)),
    })
}
