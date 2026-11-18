//! `POST /api/v2/chat/{pane}/summarize`: asks Claude, run once on its own,
//! where the chat stands. It answers at once; the summary
//! reaches every phone's list on `home` when the agent is done.

use crate::{
    hub::{RowSummary, row_summary},
    web::{ApiError, Device, Shared, now_ms},
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

/// `202` with an empty object, which a phone's JSON client reads.
type Accepted = (StatusCode, Json<serde_json::Value>);

fn accepted() -> Accepted {
    (StatusCode::ACCEPTED, Json(serde_json::json!({})))
}

pub(crate) async fn summarize(
    State(state): State<Shared>,
    _device: Device,
    Path(pane): Path<String>,
) -> Result<Accepted, ApiError> {
    state.hub.layout().chat(&pane).ok_or(ApiError::NotFound)?;
    let link = state.hub.chat(&pane).ok_or(ApiError::NotFound)?;
    let (transcript, _) = link.base();
    // The summary describes the chat as it was read, so it carries that
    // time: a turn that ends while Claude works is newer, and its row
    // shows the reply rather than the summary.
    let read_ms = now_ms();
    let input = row_summary::excerpt_of(&transcript);
    if input.is_empty() {
        return Err(ApiError::Invalid("nothing has been said in this chat yet"));
    }
    let slot = row_summary::Slot::take()
        .map_err(|_| ApiError::Invalid("too many summaries are running; try again in a moment"))?;
    if !link.begin_summary(now_ms()) {
        return Ok(accepted());
    }
    tokio::spawn(async move {
        let answer = tokio::task::spawn_blocking(move || {
            let _slot = slot;
            row_summary::run(&input)
        })
        .await;
        let answer = answer.unwrap_or(Err(row_summary::Error::Failed));
        link.finish_summary(finished(answer, read_ms));
    });
    Ok(accepted())
}

/// The row's summary from Claude's answer to the transcript read at
/// `read_ms`.
fn finished(answer: Result<String, row_summary::Error>, read_ms: u64) -> RowSummary {
    match answer {
        Ok(text) => RowSummary::Ready {
            text,
            at_ms: read_ms,
        },
        Err(error) => RowSummary::Failed {
            reason: error.to_string(),
            at_ms: read_ms,
        },
    }
}

#[cfg(test)]
mod tests;
