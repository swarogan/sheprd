//! The repository memory a chat draws on: opened once in this process,
//! offered to the agent as an index with the first prompt and as a few
//! recalled entries with each prompt after, each entry at most once.
//!
//! Memory only helps: a chat whose memory cannot be opened or read goes
//! without it. `sheprd-acp` draws its chat view on this terminal and keeps
//! no log, so there is nowhere to report that but the agent's own
//! `sheprd memory` calls, which say why.
use sheprd_memory::{Checkout, Memory, Scope};
use std::{collections::HashSet, path::Path};

/// The index's budget in the first note, in characters.
const INDEX_CHARS: usize = 2000;
/// The most entries recalled with one prompt.
const RECALLED: usize = 3;
/// The longest recalled line, in characters.
const LINE: usize = 300;
/// Prompts shorter than this, in characters, recall nothing.
const MIN_PROMPT: usize = 10;

#[derive(Debug)]
pub(crate) struct Recall {
    memory: Memory,
    checkout: Checkout,
    /// Entries already shown in this conversation.
    offered: HashSet<i64>,
}

impl Recall {
    /// The memory of the repository `cwd` is in, or `None` outside a Git
    /// checkout or when it cannot be opened: a chat then goes without.
    pub(crate) fn open(cwd: &Path) -> Option<Self> {
        let opened = Checkout::find(cwd).and_then(|checkout| {
            let path = sheprd_memory::path(&checkout.common_dir)
                .ok_or(sheprd_memory::Error::NoStateDirectory)?;
            Ok(Self {
                memory: Memory::open(&path)?,
                checkout,
                offered: HashSet::new(),
            })
        });
        opened.ok()
    }

    #[cfg(test)]
    pub(crate) fn with(memory: Memory, checkout: Checkout) -> Self {
        Self {
            memory,
            checkout,
            offered: HashSet::new(),
        }
    }

    /// Pinned entries, this branch's, and the most recently used, one line
    /// each, or `None` when the repository has none.
    pub(crate) fn index(&mut self) -> Option<String> {
        let branch = self.checkout.branch();
        let index = self.memory.index(branch.as_deref(), INDEX_CHARS).ok()?;
        if index.lines.is_empty() {
            return None;
        }
        self.offered.extend(&index.ids);
        Some(index.text())
    }

    /// Lines for the entries that bear on `prompt` and were not offered
    /// yet, or `None`.
    pub(crate) fn recall(&mut self, prompt: &str) -> Option<String> {
        let prompt = prompt.trim();
        if prompt.chars().count() < MIN_PROMPT || prompt.starts_with('/') {
            return None;
        }
        let branch = self.checkout.branch();
        let scope = Scope::Here {
            branch: branch.as_deref(),
        };
        let found = self.memory.recall(scope, prompt, RECALLED * 2).ok()?;
        let lines: Vec<String> = found
            .into_iter()
            .filter(|entry| self.offered.insert(entry.id))
            .take(RECALLED)
            .map(|entry| entry.line(LINE))
            .collect();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}
