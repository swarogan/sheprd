use std::path::PathBuf;

/// Why the agent, or one of its commands, failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("another agent is already running on {}", path.display())]
    AlreadyRunning { path: PathBuf },
    #[error("{} holds more than {} desktops", path.display(), crate::authorized::MAX_ENTRIES)]
    AuthorizedFull { path: PathBuf },
    #[error("{} line {line} is not `DIAL_KEY TUNNEL_KEY [LABEL]`", path.display())]
    AuthorizedLine { line: usize, path: PathBuf },
    #[error("the command line is not valid")]
    Cli(#[from] crate::cli::Error),
    #[error("the relay could not be reached")]
    Client(#[from] sheprd_relay::client::Error),
    #[error("could not reach the SSH server this agent forwards to")]
    Forward(#[source] std::io::Error),
    #[error("{}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("no home directory to keep the agent's state in")]
    NoHome,
    #[error("could not start the async runtime")]
    Runtime(#[source] std::io::Error),
    #[error("a session's stream broke")]
    Splice(#[source] std::io::Error),
    #[error("{} is not the agent's status", path.display())]
    StatusFormat {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{} is not the agent's keys", path.display())]
    StoredKey { path: PathBuf },
    #[error("{} is larger than {limit} bytes", path.display())]
    TooLarge { limit: u64, path: PathBuf },
    #[error("the end-to-end tunnel failed")]
    Tunnel(#[from] sheprd_tunnel::Error),
    // keep-sorted end
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// `error` and its sources joined with `: `, for `status` and the command
/// line, where the cause matters more than the step.
#[must_use]
pub fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
