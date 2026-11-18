//! One repository's memory, open: the schema, writes, and the three reads
//! (listing, search, and the index an agent sees before it starts).
use crate::{Added, Entry, Error, Index, Kind, New, Result, Scope, safety};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use std::{path::Path, time::Duration};

/// The newest schema this build reads and writes.
pub const SCHEMA_VERSION: i64 = 1;
/// The longest text kept, in bytes: a sentence or two. The rest goes in
/// details.
pub const MAX_TEXT: usize = 500;
/// The longest details kept, in bytes.
pub const MAX_DETAILS: usize = 16 * 1024;
/// The longest name, in bytes.
pub const MAX_NAME: usize = 64;
/// The most entries one repository keeps; the least recently used unpinned
/// ones go first.
pub const MAX_ENTRIES: u32 = 2000;
/// The most entries one repository may pin.
pub const MAX_PINNED: u32 = 50;
/// The most entries one listing or search returns.
pub const MAX_LIMIT: usize = 200;

/// How long a write waits for another process's transaction.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// IDs use AUTOINCREMENT so that one is never reused: a `forget` from an
/// agent that saw an entry before another forgot it cannot remove a newer
/// one. The FTS index is an external-content table the triggers keep in
/// step, weighted by column at query time.
const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    branch TEXT,
    name TEXT,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    details TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    uses INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    used_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS memories_scope_text ON memories(ifnull(branch, ''), text);
CREATE UNIQUE INDEX IF NOT EXISTS memories_scope_name
    ON memories(ifnull(branch, ''), name) WHERE name IS NOT NULL;
CREATE INDEX IF NOT EXISTS memories_rank ON memories(pinned, used_at);
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts
    USING fts5(name, text, details, content = 'memories', content_rowid = 'id');
CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, name, text, details)
        VALUES (new.id, new.name, new.text, new.details);
END;
CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, name, text, details)
        VALUES ('delete', old.id, old.name, old.text, old.details);
END;
CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE OF name, text, details ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, name, text, details)
        VALUES ('delete', old.id, old.name, old.text, old.details);
    INSERT INTO memories_fts(rowid, name, text, details)
        VALUES (new.id, new.name, new.text, new.details);
END;
";

const COLUMNS: &str = "m.id, m.branch, m.name, m.kind, m.text, m.details, m.pinned, m.uses, \
     m.created_at, m.used_at, m.updated_at";

/// Entries `?1 = all` or of the repository and branch `?2`.
const IN_SCOPE: &str = "(?1 OR m.branch IS NULL OR m.branch = ?2)";

/// One repository's memory, open.
#[derive(Debug)]
pub struct Memory {
    connection: Connection,
}

impl Memory {
    /// Opens `path`, creating the private directory and file when missing,
    /// and migrates it to [`SCHEMA_VERSION`].
    pub fn open(path: &Path) -> Result<Self> {
        sheprd_client::private_file::prepare(path)?;
        Self::with(Connection::open(path)?)
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> Result<Self> {
        Self::with(Connection::open_in_memory()?)
    }

    #[cfg(test)]
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    fn with(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(BUSY_TIMEOUT)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let found: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if found > SCHEMA_VERSION {
            return Err(Error::NewerSchema {
                found,
                known: SCHEMA_VERSION,
            });
        }
        if found < 1 {
            transaction.execute_batch(SCHEMA_V1)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        transaction.commit()?;
        Ok(Self { connection })
    }

    /// Keeps `new` at `now` (Unix seconds): updates the entry of its name
    /// in that scope, or marks the same text there used, or adds it.
    pub fn add(&mut self, new: &New<'_>, now: i64) -> Result<Added> {
        let text = checked(new.text, "text", MAX_TEXT)?;
        let details = new
            .details
            .map(|details| checked(details, "details", MAX_DETAILS))
            .transpose()?
            .filter(|details| !details.is_empty());
        let name = new.name.map(name).transpose()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let same_text = find(&transaction, "text", new.branch, text)?;
        let named = match name {
            Some(name) => find(&transaction, "name", new.branch, name)?,
            None => None,
        };
        let added = match (named, same_text) {
            (Some(id), Some(other)) if other != id => return Err(Error::SameText { id: other }),
            (Some(id), _) => {
                transaction.execute(
                    "UPDATE memories SET kind = ?2, text = ?3, details = ?4, \
                     pinned = max(pinned, ?5), used_at = ?6, updated_at = ?6 WHERE id = ?1",
                    params![id, new.kind.as_str(), text, details, new.pinned, now],
                )?;
                Added::Updated(id)
            }
            (None, Some(id)) => {
                transaction.execute(
                    "UPDATE memories SET kind = ?2, pinned = max(pinned, ?3), used_at = ?4, \
                     name = ifnull(name, ?5), details = ifnull(?6, details) WHERE id = ?1",
                    params![id, new.kind.as_str(), new.pinned, now, name, details],
                )?;
                Added::Again(id)
            }
            (None, None) => {
                transaction.execute(
                    "INSERT INTO memories \
                     (branch, name, kind, text, details, pinned, created_at, used_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7)",
                    params![
                        new.branch,
                        name,
                        new.kind.as_str(),
                        text,
                        details,
                        new.pinned,
                        now
                    ],
                )?;
                Added::New(transaction.last_insert_rowid())
            }
        };
        if new.pinned {
            within_pins(&transaction)?;
        }
        evict(&transaction)?;
        transaction.commit()?;
        Ok(added)
    }

    /// Entry `id`, with its details, marked used at `now`.
    pub fn get(&mut self, id: i64, now: i64) -> Result<Option<Entry>> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        touch(&transaction, id, now)?;
        let entry = transaction
            .query_row(
                &format!("SELECT {COLUMNS} FROM memories m WHERE m.id = ?1"),
                [id],
                entry,
            )
            .optional()?;
        transaction.commit()?;
        Ok(entry)
    }

    /// Pins or unpins entry `id`; false when there is none.
    pub fn pin(&mut self, id: i64, pinned: bool) -> Result<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let found = transaction.execute(
            "UPDATE memories SET pinned = ?2 WHERE id = ?1",
            params![id, pinned],
        )? > 0;
        if pinned {
            within_pins(&transaction)?;
        }
        transaction.commit()?;
        Ok(found)
    }

    /// Forgets entry `id`; false when there was none.
    pub fn forget(&mut self, id: i64) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM memories WHERE id = ?1", [id])?
            > 0)
    }

    /// The entries `scope` sees, pinned first, then most recently used, at
    /// most `limit`.
    pub fn list(&self, scope: Scope<'_>, limit: usize) -> Result<Vec<Entry>> {
        let (all, branch) = scope_params(scope);
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM memories m WHERE {IN_SCOPE} \
             ORDER BY m.pinned DESC, m.used_at DESC, m.id DESC LIMIT ?3"
        ))?;
        let rows = statement.query_map(params![all, branch, bounded(limit)], entry)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The entries `scope` sees that match `query`, best first, at most
    /// `limit`: those holding every word when there are any, else those
    /// holding one. Words match as prefixes, and FTS5's own syntax is never
    /// taken from the caller. With `touch`, each found is marked used then;
    /// a recall that only offers entries leaves them as they were.
    pub fn search(
        &mut self,
        scope: Scope<'_>,
        query: &str,
        limit: usize,
        touch: Option<i64>,
    ) -> Result<Vec<Entry>> {
        let words = words(query);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut found = matching(&transaction, scope, &words.join(" AND "), limit)?;
        if found.is_empty() && words.len() > 1 {
            found = matching(&transaction, scope, &words.join(" OR "), limit)?;
        }
        if let Some(now) = touch {
            for entry in &found {
                self::touch(&transaction, entry.id, now)?;
            }
        }
        transaction.commit()?;
        Ok(found)
    }

    /// What an agent on `branch` should see before it starts, within
    /// `budget` characters: pinned entries, then the branch's own (handoffs
    /// first), then the repository's most recently used, one line each.
    pub fn index(&self, branch: Option<&str>, budget: usize) -> Result<Index> {
        let scope = Scope::Here { branch };
        let (all, branch_param) = scope_params(scope);
        let total: i64 = self.connection.query_row(
            &format!("SELECT count(*) FROM memories m WHERE {IN_SCOPE}"),
            params![all, branch_param],
            |row| row.get(0),
        )?;
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM memories m WHERE {IN_SCOPE} \
             ORDER BY m.pinned DESC, (m.branch IS NOT NULL) DESC, (m.kind = 'handoff') DESC, \
             m.used_at DESC, m.id DESC LIMIT ?3"
        ))?;
        let rows = statement.query_map(params![all, branch_param, bounded(MAX_LIMIT)], entry)?;
        let mut index = Index {
            lines: Vec::new(),
            ids: Vec::new(),
            total: usize::try_from(total).unwrap_or(0),
        };
        let mut used = 0;
        for row in rows {
            let entry = row?;
            let line = entry.line(LINE);
            used += line.chars().count() + 1;
            if used > budget {
                break;
            }
            index.lines.push(line);
            index.ids.push(entry.id);
        }
        Ok(index)
    }

    /// Entries `scope` sees that bear on `prompt`, best first, at most
    /// `limit`, for a recall that offers them without marking them used.
    /// Short and common words are left out, and an entry must hold at least
    /// two of the prompt's other words (or its only one), so a long prompt
    /// does not match everything.
    pub fn recall(&mut self, scope: Scope<'_>, prompt: &str, limit: usize) -> Result<Vec<Entry>> {
        let keywords = crate::recall::keywords(prompt);
        if keywords.is_empty() {
            return Ok(Vec::new());
        }
        let found = self.search(scope, &keywords.join(" "), limit.saturating_mul(4), None)?;
        let needed = keywords.len().min(2);
        Ok(found
            .into_iter()
            .filter(|entry| crate::recall::matched(entry, &keywords) >= needed)
            .take(limit)
            .collect())
    }
}

/// The longest index line, in characters.
const LINE: usize = 240;

/// `text` trimmed, not empty, at most `max` bytes, and safe to show.
fn checked<'a>(text: &'a str, what: &'static str, max: usize) -> Result<&'a str> {
    let text = text.trim();
    if what == "text" && text.is_empty() {
        return Err(Error::Empty);
    }
    if text.len() > max {
        return Err(Error::TooLong { what, max });
    }
    safety::check(text)?;
    Ok(text)
}

/// A name: lowercase letters, digits, and `-_./`, as `build-commands`.
fn name(name: &str) -> Result<&str> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_./".contains(&byte)
        });
    valid.then_some(name).ok_or(Error::Name { max: MAX_NAME })
}

/// The entry whose `column` is `value` in `branch`'s scope.
fn find(
    transaction: &Transaction<'_>,
    column: &str,
    branch: Option<&str>,
    value: &str,
) -> Result<Option<i64>> {
    Ok(transaction
        .query_row(
            &format!(
                "SELECT id FROM memories WHERE ifnull(branch, '') = ifnull(?1, '') AND {column} = ?2"
            ),
            params![branch, value],
            |row| row.get(0),
        )
        .optional()?)
}

fn touch(transaction: &Transaction<'_>, id: i64, now: i64) -> Result<()> {
    transaction.execute(
        "UPDATE memories SET used_at = ?2, uses = uses + 1 WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

fn within_pins(transaction: &Transaction<'_>) -> Result<()> {
    let pinned: u32 =
        transaction.query_row("SELECT count(*) FROM memories WHERE pinned", [], |row| {
            row.get(0)
        })?;
    if pinned > MAX_PINNED {
        return Err(Error::TooManyPinned {
            max: MAX_PINNED as usize,
        });
    }
    Ok(())
}

/// Drops the least recently used unpinned entries past [`MAX_ENTRIES`].
fn evict(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute(
        "DELETE FROM memories WHERE id IN (SELECT id FROM memories WHERE NOT pinned \
         ORDER BY used_at DESC, id DESC LIMIT -1 \
         OFFSET max(0, ?1 - (SELECT count(*) FROM memories WHERE pinned)))",
        [MAX_ENTRIES],
    )?;
    Ok(())
}

fn matching(
    transaction: &Transaction<'_>,
    scope: Scope<'_>,
    query: &str,
    limit: usize,
) -> Result<Vec<Entry>> {
    let (all, branch) = scope_params(scope);
    // Name hits weigh most, then text, then details.
    let mut statement = transaction.prepare(&format!(
        "SELECT {COLUMNS} FROM memories_fts JOIN memories m ON m.id = memories_fts.rowid \
         WHERE memories_fts MATCH ?4 AND {IN_SCOPE} \
         ORDER BY bm25(memories_fts, 3.0, 2.0, 1.0), m.pinned DESC, m.used_at DESC LIMIT ?3"
    ))?;
    let rows = statement.query_map(params![all, branch, bounded(limit), query], entry)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn bounded(limit: usize) -> i64 {
    i64::try_from(limit.clamp(1, MAX_LIMIT)).unwrap_or(1)
}

fn scope_params(scope: Scope<'_>) -> (bool, Option<&str>) {
    match scope {
        Scope::All => (true, None),
        Scope::Here { branch } => (false, branch),
    }
}

/// Each word of `query` quoted as an FTS5 prefix term, so operators and
/// punctuation in it are only text.
fn words(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|word| !word.is_empty())
        .take(32)
        .map(|word| format!("\"{word}\"*"))
        .collect()
}

fn entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let kind: String = row.get(3)?;
    Ok(Entry {
        id: row.get(0)?,
        branch: row.get(1)?,
        name: row.get(2)?,
        // A kind a newer build wrote reads as a fact rather than failing.
        kind: kind.parse().unwrap_or(Kind::Fact),
        text: row.get(4)?,
        details: row.get(5)?,
        pinned: row.get(6)?,
        uses: row.get(7)?,
        created: row.get(8)?,
        used: row.get(9)?,
        updated: row.get(10)?,
    })
}
