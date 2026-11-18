//! The recorded frames (`fixtures/`), shared by the tests.

use serde_json::Value;

pub(crate) const SNAPSHOTS: &str = include_str!("../fixtures/snapshots.json");
pub(crate) const LIVE: &str = include_str!("../fixtures/live-turn.json");
/// The recorded thread.
pub(crate) const THREAD: &str = "mcp:a25d344c-e7a9-41ec-a6b4-33cb6f752f91";

/// Every server frame of the fixture `text`.
pub(crate) fn frames(text: &str) -> Vec<Value> {
    serde_json::from_str(text).unwrap_or_default()
}

/// The values of request `id`'s chunks in the fixture `text`, in order.
pub(crate) fn values(text: &str, id: &str) -> Vec<Value> {
    frames(text)
        .into_iter()
        .filter(|frame| frame["_tag"] == "Chunk" && frame["requestId"] == id)
        .flat_map(|frame| frame["values"].as_array().cloned().unwrap_or_default())
        .collect()
}
