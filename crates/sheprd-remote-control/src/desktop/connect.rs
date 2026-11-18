//! `relay-connect`: the `ProxyCommand` `ssh` runs for a machine added over
//! Bluetooth (`sheprd_client::relay_host`). It dials the machine's route
//! on the first of its relays that lets this computer's key through, runs
//! the tunnel against the identity pinned when pairing, and carries bytes
//! between its stdio and the tunnel; SSH's own encryption and host key
//! check run inside, unchanged.

use super::{Error, Keys, Result, record::Record};
use sheprd_relay::{Endpoint, RouteId, client};
use std::{path::Path, time::Duration};
use tokio::io::join;

/// How long reaching one relay and dialing through it may take before the
/// next is tried.
const ATTEMPT: Duration = Duration::from_secs(10);

/// Carries `ssh`'s connection to `host`, `r-ID.sheprd.invalid`, until
/// either side closes.
pub(crate) async fn connect(state_dir: &Path, host: &str) -> Result<()> {
    let (connection, mut tunnel) = open(state_dir, host).await?;
    let mut stdio = join(tokio::io::stdin(), tokio::io::stdout());
    let copied = tokio::io::copy_bidirectional(&mut stdio, &mut tunnel.io).await;
    drop(connection);
    copied.map(drop).map_err(Error::Splice)
}

/// The tunnel to `host`, through the first relay that lets this computer
/// through, and the relay connection carrying it.
pub(crate) async fn open(
    state_dir: &Path,
    host: &str,
) -> Result<(quinn::Connection, sheprd_tunnel::stream::Tunnel)> {
    let id = sheprd_client::relay_host::host_id(host)
        .ok_or_else(|| Error::Record(host.chars().take(80).collect()))?
        .to_owned();
    let state = state_dir.to_owned();
    let (record, keys) = tokio::task::spawn_blocking(move || {
        Ok::<_, Error>((Record::load(&state, &id)?, Keys::load_or_create(&state)?))
    })
    .await
    .map_err(|_| Error::Cancelled)??;
    let identity = record.tunnel_identity()?;
    let route = RouteId::parse(&record.route).ok_or(Error::RelayInfo)?;
    let mut last = None;
    for relay in &record.relays {
        let Ok(endpoint) = Endpoint::parse(relay) else {
            continue;
        };
        let attempt = async {
            let connection = client::reach(&endpoint).await?;
            let halves = client::dial(&connection, route, &keys.dial).await?;
            Ok::<_, client::Error>((connection, halves))
        };
        let (connection, (send, recv)) = match tokio::time::timeout(ATTEMPT, attempt).await {
            Ok(Ok(dialed)) => dialed,
            Ok(Err(error)) => {
                last = Some(error);
                continue;
            }
            Err(_) => continue,
        };
        let (initiator, first) =
            sheprd_tunnel::Initiator::session(&keys.tunnel, &identity, route.as_bytes())?;
        let tunnel = tokio::time::timeout(
            ATTEMPT,
            sheprd_tunnel::stream::connect(join(recv, send), initiator, &first, identity.x25519()),
        )
        .await
        .map_err(|_| Error::Tunnel(sheprd_tunnel::Error::Closed))??;
        return Ok((connection, tunnel));
    }
    Err(last.map_or(Error::NoRelays, Error::Relay))
}
