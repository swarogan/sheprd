use std::path::PathBuf;

/// What can go wrong talking to a T3 Code server.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server answered an RPC with a failure; `cause` is its own
    /// description, kept as a diagnostic.
    #[error("T3 Code refused {tag}: {cause}")]
    Call { cause: String, tag: String },
    /// The server could not be reached.
    #[error("could not reach T3 Code")]
    Connect(#[source] std::io::Error),
    /// The socket closed before the answer arrived.
    #[error("the T3 Code connection closed")]
    Closed,
    /// Nothing arrived within the read timeout. The socket is still
    /// usable: a partly read message is kept for the next read.
    #[error("T3 Code sent nothing for a while")]
    Idle,
    #[error("could not read {path}")]
    Discovery {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// No server on this computer said where it listens.
    #[error("no T3 Code server is running here")]
    NoServer,
    #[error("T3 Code's HTTP API failed")]
    Http(#[source] Box<ureq::Error>),
    #[error("T3 Code sent JSON this client cannot read")]
    Json(#[from] serde_json::Error),
    /// The server speaks another orchestration protocol than this client.
    #[error("T3 Code speaks orchestration protocol {found}; this client speaks {expected}")]
    Protocol { expected: u32, found: u32 },
    /// An HTTP call answered with an error status.
    #[error("T3 Code answered {path} with HTTP {status}")]
    Status { path: &'static str, status: u16 },
    /// An address of a kind this client cannot open yet, such as `wss://`.
    #[error("T3 Code addresses with `{0}` are not supported yet")]
    Scheme(String),
    /// The server issued a token this client cannot use, such as one bound
    /// to a proof key.
    #[error("T3 Code issued a {0} token; this client uses Bearer tokens")]
    TokenType(String),
    #[error(transparent)]
    Token(#[from] crate::token::Error),
    /// A message larger than this client accepts.
    #[error("T3 Code sent a message over {limit} bytes")]
    TooLarge { limit: usize },
    /// The server's address cannot be used.
    #[error("invalid T3 Code address")]
    Url(#[from] url::ParseError),
    #[error("T3 Code's WebSocket failed")]
    WebSocket(#[source] Box<tungstenite::Error>),
}

impl From<ureq::Error> for Error {
    fn from(error: ureq::Error) -> Self {
        Self::Http(Box::new(error))
    }
}

impl From<tungstenite::Error> for Error {
    fn from(error: tungstenite::Error) -> Self {
        match error {
            tungstenite::Error::Capacity(_) => Self::TooLarge {
                limit: crate::connection::MAX_MESSAGE,
            },
            tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
                Self::Closed
            }
            tungstenite::Error::Io(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Self::Idle
            }
            error => Self::WebSocket(Box::new(error)),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
