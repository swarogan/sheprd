//! One relay: connect over QUIC, listen on this computer's route, name the
//! paired devices' dial keys, and hand each dial to [`super::serve`];
//! reconnect with backoff when anything breaks.

use super::{Error, Relays, Result, Served, Status, serve};
use crate::web::Shared;
use sheprd_relay::{
    Endpoint,
    client::{self, Listener},
    wire::Key,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

/// Reconnects wait this long at first, doubling to [`LONGEST`].
const FIRST: Duration = Duration::from_secs(1);
const LONGEST: Duration = Duration::from_secs(60);
/// How often a listening relay's round trip is read again.
const RTT_EVERY: Duration = Duration::from_secs(30);

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
            _ = stopped.wait_for(|stop| *stop) => {
                relays.set_rtt(&endpoint, None);
                return;
            }
        };
        relays.set_rtt(&endpoint, None);
        if let Err(error) = result {
            // Another listener with this computer's key took the route: come
            // back slowly, so the two do not take it from each other forever.
            if matches!(error, Error::Client(client::Error::Replaced)) {
                wait = LONGEST;
            }
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
    let connection = client::reach(endpoint).await?;
    let mut listener = Listener::listen(connection, &relays.keys.route, b"").await?;
    let mut changed = relays.changed.subscribe();
    let mut pairing = relays.pairing_changed.subscribe();
    let mut revoked = state.auth().watch_revocations();
    listener.set_dial_keys(dial_keys(state)).await?;
    add_pairing(relays, &mut listener).await;
    // The round trip first, so whoever sees `Listening` sees it too.
    relays.set_rtt(endpoint, Some(listener.rtt()));
    relays.set_status(endpoint, Status::Listening { rtt_ms: None });
    tracing::info!(relay = %endpoint, rtt = ?listener.rtt(), "listening on a relay");
    // Listening worked: the next failure starts the backoff over.
    *wait = FIRST;
    let mut measure = tokio::time::interval(RTT_EVERY);
    loop {
        tokio::select! {
            // Only taking the stream waits here, which is cancel-safe: its
            // header is read in its own task, so another event (the round
            // trip timer, new keys) never drops a dial half read.
            pending = listener.next_stream() => {
                let pending = pending?;
                let relays = Arc::clone(relays);
                let route = listener.route();
                let served = served.clone();
                tokio::spawn(async move {
                    match pending.incoming().await {
                        Ok(incoming) => serve::session(relays, incoming, route, served).await,
                        Err(error) => tracing::debug!(error = %error, "a relay dial ended before its header"),
                    }
                });
            }
            _ = changed.changed() => listener.set_dial_keys(dial_keys(state)).await?,
            _ = pairing.changed() => add_pairing(relays, &mut listener).await,
            _ = revoked.changed() => listener.set_dial_keys(dial_keys(state)).await?,
            _ = measure.tick() => relays.set_rtt(endpoint, Some(listener.rtt())),
            error = listener.closed() => return Err(Error::Client(error)),
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
