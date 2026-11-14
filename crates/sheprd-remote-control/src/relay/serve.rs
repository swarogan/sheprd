//! One dial: the end-to-end tunnel, admitting only a paired device's
//! tunnel key, then TLS with the certificate for `127.0.0.1`, then the
//! API, as axum serves it on the network.

use super::{Relays, Result, Served};
use crate::web::{Peer, Shared};
use axum::{Extension, extract::ConnectInfo};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use sha2::{Digest, Sha256};
use sheprd_relay::{RouteId, client::Incoming};
use sheprd_tunnel::{Responder, stream};
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::io::join;

/// How long a dialer has to finish the handshake.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// Serves one dial until either side closes. Failures end the session
/// alone; the log names no address or key.
pub(super) async fn session(
    relays: Arc<Relays>,
    incoming: Incoming,
    route: RouteId,
    served: Served,
) {
    if let Err(error) = serve(&relays, incoming, route, served).await {
        tracing::debug!(error = %error, "a relay session ended");
    }
}

async fn serve(relays: &Relays, incoming: Incoming, route: RouteId, served: Served) -> Result<()> {
    let Served { router, state, tls } = served;
    let state = &state;
    let tunnel_keys = &relays.keys.tunnel;
    // The open pairing dial runs the pairing handshake, whose PSK is the
    // token sealed to the nearby phone: any phone that holds it may claim,
    // as over the network. Every other dial must be a paired device's.
    let pairing = relays.pairing_secret(&incoming.key);
    let responder = match &pairing {
        Some(secret) => Responder::pairing(tunnel_keys, route.as_bytes(), secret)?,
        None => Responder::session(tunnel_keys, route.as_bytes())?,
    };
    let io = join(incoming.recv, incoming.send);
    let paired = |remote: &[u8; 32]| pairing.is_some() || paired(state, remote);
    let accepted = tokio::time::timeout(
        HANDSHAKE,
        stream::accept(io, tunnel_keys, responder, paired),
    )
    .await
    .map_err(|_| sheprd_tunnel::Error::Closed)??;
    // Rate limits and logs key callers by address; each device gets its own
    // private one, derived from its tunnel key, so one cannot spend
    // another's.
    let peer = SocketAddr::new(address_for(&accepted.remote), 0);
    let service = TowerToHyperService::new(router.layer(Extension(ConnectInfo(Peer(peer)))));
    let tls = tokio::time::timeout(
        HANDSHAKE,
        tokio_rustls::TlsAcceptor::from(tls).accept(accepted.io),
    )
    .await
    .map_err(|_| sheprd_tunnel::Error::Closed)?
    .map_err(|_| sheprd_tunnel::Error::Closed)?;
    let served = hyper::server::conn::http1::Builder::new()
        .serve_connection(TokioIo::new(tls), service)
        .with_upgrades()
        .await;
    if let Err(error) = served {
        tracing::debug!(error = %error, "a relay connection ended");
    }
    Ok(())
}

/// Whether `remote` is a paired device's tunnel key.
fn paired(state: &Shared, remote: &[u8; 32]) -> bool {
    state
        .auth()
        .devices()
        .records()
        .iter()
        .filter_map(|record| record.relay.as_ref()?.tunnel_key())
        .any(|key| key == *remote)
}

/// A unique local IPv6 address standing for the device with this tunnel
/// key: `fd`, then 15 bytes of its SHA-256.
fn address_for(remote: &[u8; 32]) -> IpAddr {
    let digest = Sha256::digest(remote);
    let mut bytes = [0; 16];
    bytes[0] = 0xfd;
    bytes[1..].copy_from_slice(&digest[..15]);
    IpAddr::V6(Ipv6Addr::from(bytes))
}
