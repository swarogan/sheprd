//! Agent memory: what agents learned about a repository, kept for the next
//! agent in any of its worktrees, tabs, or chats on the same host.
//!
//! One SQLite file per repository (`location`), keyed by the Git common
//! directory every worktree shares (`checkout`), holds short entries with
//! optional details. Search is FTS5, ranked by relevance, then pins, then
//! recent use; pinned entries are never evicted, and the least recently
//! used of the rest go once a repository holds [`MAX_ENTRIES`]. Writes that
//! look like secrets or prompt injection are refused (`safety`), since
//! entries are shown to later agents.
//!
//! Both `sheprd memory` and `sheprd-acp`, which recalls entries into a
//! chat's prompts, open the file in their own process. Every call blocks on
//! disk I/O; the app never runs one on the UI thread. No GPUI, no runtime.

#![cfg_attr(test, allow(clippy::disallowed_methods))]

mod checkout;
mod error;
mod location;
mod model;
mod recall;
mod safety;
mod store;
#[cfg(test)]
mod tests;

pub use checkout::Checkout;
pub use error::{Error, Result};
pub use location::{path, path_in};
pub use model::{Added, Entry, Index, KINDS, Kind, New, Scope};
pub use safety::Unsafe;
pub use store::{
    MAX_DETAILS, MAX_ENTRIES, MAX_LIMIT, MAX_NAME, MAX_PINNED, MAX_TEXT, Memory, SCHEMA_VERSION,
};
