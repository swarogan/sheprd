use std::path::PathBuf;

/// What can go wrong keeping a token.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}; run `sheprd-t3 pair` first")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a token file")]
    Invalid { path: PathBuf },
    /// The token was issued by another server than the one answering now.
    #[error("{path} holds a token for another T3 Code server; pair again with this one")]
    OtherServer { path: PathBuf },
}
