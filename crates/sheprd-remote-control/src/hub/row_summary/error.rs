//! Why the agent gave no summary; shown on the chat's row as it says.

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    // keep-sorted start
    #[error("too many summaries are running; try again in a moment")]
    Busy,
    #[error("the agent answered nothing")]
    Empty,
    #[error("the agent stopped with an error")]
    Failed,
    #[error("no agent that can summarize is installed on this computer")]
    NoAgent,
    #[error("the agent could not start")]
    Start(#[source] std::io::Error),
    #[error("the agent took too long to answer")]
    TimedOut,
    // keep-sorted end
}

impl Error {
    /// How a request that ran an agent answers this failure.
    pub(crate) fn status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::Busy => StatusCode::TOO_MANY_REQUESTS,
            Self::NoAgent => StatusCode::CONFLICT,
            Self::TimedOut => StatusCode::GATEWAY_TIMEOUT,
            Self::Empty | Self::Failed | Self::Start(_) => StatusCode::BAD_GATEWAY,
        }
    }
}
