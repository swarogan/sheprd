//! One relay: connect over QUIC, listen on the agent's route, name the
//! authorized desktops' dial keys, and hand each dial to [`crate::serve`];
//! reconnect with backoff when anything breaks.

use crate::{Error, Result, agent::Agent, agent::Relay, serve, status::RelayState};
use sheprd_relay::client::{self, Listener};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

/// Reconnects wait this long at first, doubling to [`LONGEST`].
const FIRST: Duration = Duration::from_secs(1);
const LONGEST: Duration = Duration::from_secs(60);

/// Keeps listening on `relay` until `stopped`.
pub(crate) async fn run(agent: Arc<Agent>, relay: Relay, mut stopped: watch::Receiver<bool>) {
    let mut wait = FIRST;
    loop {
        agent.set_state(&relay.endpoint, RelayState::Connecting);
        let result = tokio::select! {
            result = session(&agent, &relay, &mut wait) => result,
            _ = stopped.wait_for(|stop| *stop) => return,
        };
        if let Err(error) = result {
            // Another agent with these keys took the route: come back
            // slowly, so the two do not take it from each other forever.
            if matches!(error, Error::Client(client::Error::Replaced)) {
                wait = LONGEST;
            }
            tracing::warn!(relay = %relay.endpoint, error = %error, "relay listening failed");
            agent.set_state(
                &relay.endpoint,
                RelayState::Failed {
                    error: crate::error::chain(&error),
                },
            );
        }
        // Jitter, so a relay restarting does not see every agent at once.
        let jitter = Duration::from_millis(u64::from(random_byte()) * 4);
        tokio::select! {
            () = tokio::time::sleep(wait + jitter) => {}
            _ = stopped.wait_for(|stop| *stop) => return,
        }
        wait = (wait * 2).min(LONGEST);
    }
}

fn random_byte() -> u8 {
    let mut byte = [0];
    let _ = getrandom::fill(&mut byte);
    byte[0]
}

/// One connection's life: listen, name keys, serve dials, until it ends.
async fn session(agent: &Arc<Agent>, relay: &Relay, wait: &mut Duration) -> Result<()> {
    let connection = client::reach(&relay.endpoint).await?;
    let mut listener = Listener::listen(connection, &agent.keys.route, &relay.token).await?;
    let mut authorized = agent.authorized();
    let keys = authorized.borrow_and_update().dial_keys();
    listener.set_dial_keys(keys).await?;
    agent.set_state(&relay.endpoint, RelayState::Listening);
    tracing::info!(relay = %relay.endpoint, "listening on a relay");
    // Listening worked: the next failure starts the backoff over.
    *wait = FIRST;
    loop {
        tokio::select! {
            // Only taking the stream waits here, which is cancel-safe: its
            // header is read in the session's own task, so a change to the
            // desktops never drops a dial half read.
            pending = listener.next_stream() => {
                tokio::spawn(serve::session(Arc::clone(agent), pending?, listener.route()));
            }
            changed = authorized.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
                let keys = authorized.borrow_and_update().dial_keys();
                listener.set_dial_keys(keys).await?;
            }
            error = listener.closed() => return Err(error.into()),
        }
    }
}
