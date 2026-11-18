//! `POST /api/v2/chat/{pane}/title`: asks Claude, run once on its own, for a
//! short title for the chat, and answers it. The phone
//! then renames the chat, so the person sees the name before it is kept.

use crate::{
    hub::{chat_title, row_summary},
    web::{ApiError, Device, Shared},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Answer {
    title: String,
}

pub(crate) async fn title(
    State(state): State<Shared>,
    _device: Device,
    Path(pane): Path<String>,
) -> Result<Json<Answer>, ApiError> {
    state.hub.layout().chat(&pane).ok_or(ApiError::NotFound)?;
    let link = state.hub.chat(&pane).ok_or(ApiError::NotFound)?;
    let (transcript, _) = link.base();
    let input = row_summary::excerpt_of(&transcript);
    if input.is_empty() {
        return Err(ApiError::Invalid("nothing has been said in this chat yet"));
    }
    // Shares the summaries' cap, as each runs an agent program.
    let slot = row_summary::Slot::take()?;
    let title = tokio::task::spawn_blocking(move || {
        let _slot = slot;
        chat_title::run(&input)
    })
    .await
    .map_err(|_| row_summary::Error::Failed)??;
    Ok(Json(Answer { title }))
}
