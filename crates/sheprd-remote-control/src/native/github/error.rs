//! Why a phone gets no GitHub token from this computer. No text here ever
//! carries a token.

use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    // keep-sorted start
    #[error("too many GitHub sign-ins are being read; try again shortly")]
    Busy,
    #[error("that is not a GitHub host name")]
    InvalidHost,
    #[error("this computer has no GitHub sign-in for that host")]
    NoAccount,
    #[error("this computer does not share its GitHub sign-in with this device")]
    NotShared,
    #[error("too many GitHub sign-in requests; try again shortly")]
    RateLimited,
    #[error("Sheprd did not hand over its GitHub sign-in in time")]
    TimedOut,
    // keep-sorted end
}

impl Error {
    pub(crate) fn status(&self) -> StatusCode {
        match self {
            Self::Busy | Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::InvalidHost => StatusCode::BAD_REQUEST,
            Self::NoAccount => StatusCode::NOT_FOUND,
            Self::NotShared => StatusCode::FORBIDDEN,
            Self::TimedOut => StatusCode::GATEWAY_TIMEOUT,
        }
    }
}
