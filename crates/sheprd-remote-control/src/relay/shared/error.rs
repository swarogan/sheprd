use std::{io, path::PathBuf};

/// Why relays shared with this computer could not be kept.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error(transparent)]
    App(#[from] crate::app::Error),
    #[error(transparent)]
    Control(#[from] crate::control::Error),
    #[error("Could not {operation} {}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Offer(#[from] crate::control::relays::Error),
    #[error("the remote control daemon gave an answer of the wrong kind")]
    Unexpected,
    // keep-sorted end
}
