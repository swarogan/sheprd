//! Effect RPC's JSON frames, as T3 Code's `/ws` sends and expects them.
//!
//! A client sends `Request`s with ids of its choosing and acknowledges each
//! `Chunk` of a stream before the server sends the next. The server answers
//! a call with one `Exit`, and a stream with `Chunk`s until its `Exit`.
//! Frames of a kind this client does not know are kept as `Unknown` rather
//! than failing the connection.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A frame this client sends.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "_tag")]
pub enum ClientFrame {
    Request {
        id: String,
        tag: String,
        payload: Value,
        /// Effect RPC's per-request headers; this client sends none.
        headers: [(); 0],
    },
    /// Lets the server send the stream's next chunk.
    Ack {
        #[serde(rename = "requestId")]
        request_id: String,
    },
    /// Asks the server to show it is still there; it answers `Pong`.
    Ping,
    /// Ends a stream this client no longer wants.
    Interrupt {
        #[serde(rename = "requestId")]
        request_id: String,
    },
}

/// A frame the server sends.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "_tag")]
pub enum ServerFrame {
    /// The next values of a stream.
    Chunk {
        #[serde(rename = "requestId")]
        request_id: String,
        values: Vec<Value>,
    },
    /// The end of a call or a stream.
    Exit {
        #[serde(rename = "requestId")]
        request_id: String,
        exit: Exit,
    },
    /// A failure not tied to one request.
    Defect {
        defect: Value,
    },
    Pong,
    #[serde(other)]
    Unknown,
}

/// How a call or stream ended.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "_tag")]
pub enum Exit {
    Success {
        #[serde(default)]
        value: Value,
    },
    Failure {
        #[serde(default)]
        cause: Value,
    },
    #[serde(other)]
    Unknown,
}

impl Exit {
    /// The failure's own words, for a diagnostic: Effect reports a cause as
    /// a list of failures, each with a `defect` or an `error`.
    pub fn cause(&self) -> Option<String> {
        let Self::Failure { cause } = self else {
            return None;
        };
        let first = cause
            .as_array()
            .and_then(|causes| causes.first())
            .unwrap_or(cause);
        let text = first
            .get("defect")
            .or_else(|| first.get("error"))
            .map_or_else(
                || first.to_string(),
                |detail| match detail {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                },
            );
        Some(text)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
