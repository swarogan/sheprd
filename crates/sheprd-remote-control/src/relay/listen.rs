//! One relay: connect over QUIC, listen on this computer's route, name the
//! paired devices' dial keys, and hand each dial to [`super::serve`];
//! reconnect with backoff when anything breaks.

use super::{Error, Relays, Result, Served, Status, serve};
use crate::web::Shared;
use serde::Deserialize;
use sheprd_relay::{
    Endpoint,
    client::{self, Listener},
    wire::Key,
};
use std::{future::Future, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::watch;

/// Reconnects wait this long at first, doubling to [`LONGEST`].
const FIRST: Duration = Duration::from_secs(1);
const LONGEST: Duration = Duration::from_secs(60);
/// How long reading `/v1/info` may take.
const INFO_TIMEOUT: Duration = Duration::from_secs(10);
/// How long one of a relay's addresses may take to answer before the next
/// is tried. The last is given QUIC's own handshake time.
const ATTEMPT: Duration = Duration::from_secs(5);
/// How many of a relay's addresses are tried per connection.
const MAX_ADDRESSES: usize = 8;

/// Keeps listening on `endpoint` until `stopped`.
pub(super) async fn run(
    relays: Arc<Relays>,
    endpoint: Endpoint,
    served: Served,
    mut stopped: watch::Receiver<bool>,
) {
    let mut wait = FIRST;
    loop {
        relays.set_status(&endpoint, Status::Connecting);
        let result = tokio::select! {
            result = session(&relays, &endpoint, &served, &mut wait) => result,
            _ = stopped.wait_for(|stop| *stop) => return,
        };
        if let Err(error) = result {
            tracing::warn!(relay = %endpoint, error = %error, "relay listening failed");
            relays.set_status(
                &endpoint,
                Status::Failed {
                    error: error.to_string(),
                },
            );
        }
        // Jitter, so a relay restarting does not see every computer at once.
        let jitter = Duration::from_millis(u64::from(rand_byte()) * 4);
        tokio::select! {
            () = tokio::time::sleep(wait + jitter) => {}
            _ = stopped.wait_for(|stop| *stop) => return,
        }
        wait = (wait * 2).min(LONGEST);
    }
}

fn rand_byte() -> u8 {
    let mut byte = [0];
    let _ = getrandom::fill(&mut byte);
    byte[0]
}

/// One connection's life: listen, name keys, serve dials, until it ends.
async fn session(
    relays: &Arc<Relays>,
    endpoint: &Endpoint,
    served: &Served,
    wait: &mut Duration,
) -> Result<()> {
    let state = &served.state;
    let (addresses, pin) = quic(endpoint).await?;
    let connection = first_reachable(&addresses, ATTEMPT, |address| async move {
        Ok(client::connect(address, pin).await?)
    })
    .await?;
    let mut listener = Listener::listen(connection, &relays.keys.route, b"").await?;
    let mut changed = relays.changed.subscribe();
    let mut pairing = relays.pairing_changed.subscribe();
    let mut revoked = state.auth().watch_revocations();
    listener.set_dial_keys(dial_keys(state)).await?;
    add_pairing(relays, &mut listener).await;
    relays.set_status(endpoint, Status::Listening);
    tracing::info!(relay = %endpoint, "listening on a relay");
    // Listening worked: the next failure starts the backoff over.
    *wait = FIRST;
    loop {
        tokio::select! {
            incoming = listener.accept() => {
                let incoming = incoming?;
                tokio::spawn(serve::session(
                    Arc::clone(relays),
                    incoming,
                    listener.route(),
                    served.clone(),
                ));
            }
            _ = changed.changed() => listener.set_dial_keys(dial_keys(state)).await?,
            _ = pairing.changed() => add_pairing(relays, &mut listener).await,
            _ = revoked.changed() => listener.set_dial_keys(dial_keys(state)).await?,
            error = listener.closed() => return Err(Error::Client(error.into())),
        }
    }
}

/// Lets the open pairing dial through, if any. A relay refusing it (too
/// many at once) leaves listening as it was.
async fn add_pairing(relays: &Relays, listener: &mut Listener) {
    if let Some((key, seconds)) = relays.pairing_key()
        && let Err(error) = listener.add_pairing_key(key, seconds).await
    {
        tracing::warn!(error = %error, "a relay did not take the pairing key");
    }
}

/// The dial keys of every paired device that gave one.
fn dial_keys(state: &Shared) -> Vec<Key> {
    state
        .auth()
        .devices()
        .records()
        .iter()
        .filter_map(|record| record.relay.as_ref()?.dial_key())
        .collect()
}

#[derive(Deserialize)]
struct Info {
    quic: Option<Quic>,
}

#[derive(Deserialize)]
struct Quic {
    pin: String,
    port: u16,
}

/// Connects to the first of `addresses` that answers, in the resolver's
/// order: a relay with IPv6 and IPv4 records is still reached when this
/// computer has a route to only one of them. An address that fails, or
/// takes longer than `attempt` while another is left, gives way to the
/// next; the last one's failure is the one reported.
pub(super) async fn first_reachable<C, F>(
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
/// to [`MAX_ADDRESSES`]), and its certificate's pin, from its `/v1/info`
/// over HTTPS.
async fn quic(endpoint: &Endpoint) -> Result<(Vec<SocketAddr>, sheprd_relay::pin::Pin)> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let url = endpoint.info().to_string();
    let info = tokio::task::spawn_blocking(move || -> Result<Info> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(INFO_TIMEOUT))
            .max_redirects(0)
            .build()
            .into();
        let mut response = agent.get(&url).call().map_err(Error::Info)?;
        let body = response
            .body_mut()
            .with_config()
            .limit(16 * 1024)
            .read_to_vec()
            .map_err(Error::Info)?;
        serde_json::from_slice(&body).map_err(|_| Error::InfoFormat)
    })
    .await
    .map_err(|_| Error::InfoFormat)??;
    let quic = info.quic.ok_or(Error::NoQuic)?;
    let pin = URL_SAFE_NO_PAD
        .decode(quic.pin)
        .ok()
        .and_then(|pin| pin.try_into().ok())
        .ok_or(Error::InfoFormat)?;
    let host = endpoint.hostname();
    let addresses = tokio::net::lookup_host((host.as_str(), quic.port))
        .await
        .map_err(Error::Resolve)?
        .take(MAX_ADDRESSES)
        .collect();
    Ok((addresses, pin))
}
