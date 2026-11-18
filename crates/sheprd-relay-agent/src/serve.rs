//! One dial: the end-to-end tunnel, admitting only an authorized desktop's
//! tunnel key, then the SSH server, spliced until either side closes or the
//! desktop is removed from `authorized`.

use crate::{Error, Result, agent::Agent};
use sheprd_relay::{RouteId, client::Pending};
use sheprd_tunnel::{Responder, stream};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{copy_bidirectional, join},
    net::TcpStream,
};

/// How long a dialer has to finish the handshake.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// Serves one dial until it ends. Failures end the session alone; the log
/// names no address or key.
pub(crate) async fn session(agent: Arc<Agent>, pending: Pending, route: RouteId) {
    if let Err(error) = serve(&agent, pending, route).await {
        tracing::debug!(error = %error, "a relay session ended");
    }
}

async fn serve(agent: &Agent, pending: Pending, route: RouteId) -> Result<()> {
    let incoming = tokio::time::timeout(HANDSHAKE, pending.incoming())
        .await
        .map_err(|_| sheprd_tunnel::Error::Closed)??;
    let tunnel_keys = &agent.keys.tunnel;
    let responder = Responder::session(tunnel_keys, route.as_bytes())?;
    let mut authorized = agent.authorized();
    let io = join(incoming.recv, incoming.send);
    let accepted = {
        let admitted = |remote: &[u8; 32]| authorized.borrow().admits(remote);
        tokio::time::timeout(
            HANDSHAKE,
            stream::accept(io, tunnel_keys, responder, admitted),
        )
        .await
        .map_err(|_| sheprd_tunnel::Error::Closed)??
    };
    let remote = accepted.remote;
    let mut ssh = TcpStream::connect(agent.forward())
        .await
        .map_err(Error::Forward)?;
    let mut tunnel = accepted.io;
    tokio::select! {
        copied = copy_bidirectional(&mut tunnel, &mut ssh) => {
            copied.map_err(Error::Splice)?;
        }
        // The relay also ends the session when the dial key goes; this
        // covers a desktop whose tunnel key was removed under another.
        _ = authorized.wait_for(|authorized| !authorized.admits(&remote)) => {
            tracing::info!("ended a session of a desktop no longer authorized");
        }
    }
    Ok(())
}
