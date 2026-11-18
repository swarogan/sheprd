//! Reaching a relay by its endpoint: its `/v1/info` over HTTPS for the
//! QUIC port and certificate pin, every address its name resolves to, and
//! a QUIC connection to the first that answers. Shared by everything that
//! listens on or dials a relay, so each tries addresses the same way.

use super::{Error, Result, connect};
use crate::{
    Endpoint,
    endpoint::{Info, QuicInfo},
    pin::Pin,
};
use quinn::Connection;
use std::{future::Future, net::SocketAddr, time::Duration};

/// How long reading `/v1/info` may take.
const INFO_TIMEOUT: Duration = Duration::from_secs(10);
/// The most of `/v1/info` read; it is a few hundred bytes.
const INFO_LIMIT: u64 = 16 * 1024;
/// How long one of a relay's addresses may take to answer before the next
/// is tried. The last is given QUIC's own handshake time.
const ATTEMPT: Duration = Duration::from_secs(5);
/// How many of a relay's addresses are tried per connection.
const MAX_ADDRESSES: usize = 8;

/// Connects to the relay at `endpoint` over QUIC, at the first of its
/// addresses that answers.
pub async fn reach(endpoint: &Endpoint) -> Result<Connection> {
    let (addresses, pin) = quic(endpoint).await?;
    first_reachable(&addresses, ATTEMPT, |address| connect(address, pin)).await
}

/// Connects to the first of `addresses` that answers, in the resolver's
/// order: a relay with IPv6 and IPv4 records is still reached from a
/// network with a route to only one of them. An address that fails, or
/// takes longer than `attempt` while another is left, gives way to the
/// next; the last one's failure is the one reported.
pub async fn first_reachable<C, F>(
    addresses: &[SocketAddr],
    attempt: Duration,
    mut connect: impl FnMut(SocketAddr) -> F,
) -> Result<C>
where
    F: Future<Output = Result<C>>,
{
    let (last, earlier) = addresses
        .split_last()
        .ok_or_else(|| Error::Resolve(std::io::Error::other("no address")))?;
    for address in earlier {
        if let Ok(Ok(connection)) = tokio::time::timeout(attempt, connect(*address)).await {
            return Ok(connection);
        }
    }
    connect(*last).await
}

/// Where the relay's QUIC listens (every address its name resolves to, up
/// to [`MAX_ADDRESSES`]), and its certificate's pin, from its `/v1/info`.
async fn quic(endpoint: &Endpoint) -> Result<(Vec<SocketAddr>, Pin)> {
    let url = endpoint.info().to_string();
    let info = tokio::task::spawn_blocking(move || info(&url))
        .await
        .map_err(|_| Error::InfoFormat)??;
    let quic = info.quic.ok_or(Error::NoQuic)?;
    let pin = pin(&quic)?;
    let host = endpoint.hostname();
    let addresses = tokio::net::lookup_host((host.as_str(), quic.port))
        .await
        .map_err(Error::Resolve)?
        .take(MAX_ADDRESSES)
        .collect();
    Ok((addresses, pin))
}

/// Reads `/v1/info` at `url`. Blocking.
fn info(url: &str) -> Result<Info> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(INFO_TIMEOUT))
        .max_redirects(0)
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(Box::new)?;
    let body = response
        .body_mut()
        .with_config()
        .limit(INFO_LIMIT)
        .read_to_vec()
        .map_err(Box::new)?;
    serde_json::from_slice(&body).map_err(|_| Error::InfoFormat)
}

fn pin(quic: &QuicInfo) -> Result<Pin> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD
        .decode(&quic.pin)
        .ok()
        .and_then(|pin| pin.try_into().ok())
        .ok_or(Error::InfoFormat)
}

#[cfg(test)]
mod tests;
