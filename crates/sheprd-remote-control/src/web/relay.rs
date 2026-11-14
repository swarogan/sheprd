//! `GET /api/relay` tells a paired phone how to reach this computer through
//! its relays: the route, the relays, and the tunnel identity to pin.
//! `POST /api/relay` takes the phone's own relay keys, which the relays then
//! let through (`relay`). Both need a signed-in device, so a phone learns
//! them over a path it already trusts (the network or Tailscale) before it
//! ever uses a relay.

use super::{ApiError, Device, Shared};
use crate::auth::{decode_fixed, devices::RelayKeys};
use axum::{Json, extract::State};
use serde::Deserialize;

pub(super) async fn info(
    State(state): State<Shared>,
    _device: Device,
) -> Result<Json<crate::relay::Info>, ApiError> {
    let relay = state.relay.as_ref().ok_or(ApiError::NotFound)?;
    Ok(Json(relay.info()))
}

/// A phone's relay keys, base64url.
#[derive(Deserialize)]
pub(super) struct Keys {
    /// Its dial key: P-256, SEC1 compressed.
    dial: String,
    /// Its tunnel static key: X25519.
    tunnel: String,
}

pub(super) async fn enroll(
    State(state): State<Shared>,
    Device(device): Device,
    Json(keys): Json<Keys>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let relay = state.relay.clone().ok_or(ApiError::NotFound)?;
    let keys = RelayKeys {
        dial: keys.dial,
        tunnel: keys.tunnel,
    };
    if keys.dial_key().is_none() {
        return Err(ApiError::Invalid("the dial key is not a P-256 key"));
    }
    if decode_fixed::<32>(&keys.tunnel).is_err() {
        return Err(ApiError::Invalid("the tunnel key is not an X25519 key"));
    }
    let state_for_write = state.clone();
    let stored = tokio::task::spawn_blocking(move || {
        state_for_write
            .auth()
            .devices_mut()
            .set_relay(&device, keys)
    })
    .await
    .map_err(|_| ApiError::NotFound)??;
    if !stored {
        return Err(ApiError::Unauthorized);
    }
    relay.devices_changed();
    Ok(Json(serde_json::json!({})))
}
