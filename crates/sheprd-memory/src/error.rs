//! Agent memory failures.
use crate::Unsafe;
use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Memory database failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("A memory entry needs some text")]
    Empty,
    #[error("Could not run Git in {}: {source}", dir.display())]
    Git {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Unknown memory kind {0:?}")]
    Kind(String),
    #[error(
        "A memory name is 1 to {max} lowercase letters, digits, and - _ . /, such as build-commands"
    )]
    Name { max: usize },
    #[error(
        "This repository's memory is schema {found}; this sheprd understands up to {known}. Update sheprd to use it."
    )]
    NewerSchema { found: i64, known: i64 },
    #[error("Memory is kept per Git repository; this is not inside a checkout")]
    NotInCheckout,
    #[error("No state directory is available for agent memory")]
    NoStateDirectory,
    #[error(transparent)]
    Private(#[from] sheprd_client::private_file::Error),
    #[error("Entry #{id} already holds this text here")]
    SameText { id: i64 },
    #[error("A memory entry's {what} is at most {max} bytes")]
    TooLong { what: &'static str, max: usize },
    #[error("At most {max} entries may be pinned; unpin one first")]
    TooManyPinned { max: usize },
    #[error(transparent)]
    Unsafe(#[from] Unsafe),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
