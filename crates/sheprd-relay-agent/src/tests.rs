//! The agent on a real relay on loopback, forwarding to a stand-in SSH
//! server: an authorized desktop reaches it end to end; no one else does;
//! removing a desktop ends its session.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{
    agent::{Agent, Options, Relay},
    authorized::{Authorized, Entry},
    status::RelayState,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::SigningKey;
use sheprd_relay::{Endpoint, client, pin::Pin, wire::Status};
use sheprd_tunnel::{Identity, Initiator, Keys as TunnelKeys, stream};
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, join},
    net::TcpListener,
    sync::watch,
};

mod status;

/// A relay serving QUIC and HTTP on loopback: its HTTP address, and its
/// QUIC address and pin.
async fn relay() -> (SocketAddr, SocketAddr, Pin) {
    let (endpoint, info) =
        sheprd_relay::bind_quic("127.0.0.1:0".parse().unwrap(), None, None).unwrap();
    let quic = endpoint.local_addr().unwrap();
    let pin = URL_SAFE_NO_PAD
        .decode(&info.pin)
        .unwrap()
        .try_into()
        .unwrap();
    let options = sheprd_relay::Options {
        metrics: None,
        ..sheprd_relay::Options::default()
    };
    let (router, hub) = sheprd_relay::app(&options, Some(info)).unwrap();
    tokio::spawn(sheprd_relay::quic::run(endpoint, hub));
    let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http = tcp.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            tcp,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    (http, quic, pin)
}

/// A stand-in SSH server that greets each connection, then echoes.
async fn ssh_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                socket.write_all(b"SSH-2.0-test\r\n").await.unwrap();
                let mut buffer = [0; 1024];
                while let Ok(read) = socket.read(&mut buffer).await {
                    if read == 0 || socket.write_all(&buffer[..read]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    address
}

/// A desktop's keys, and its line in `authorized`.
struct Desktop {
    dial: SigningKey,
    tunnel: TunnelKeys,
}

impl Desktop {
    fn new(seed: u8) -> Self {
        Self {
            dial: SigningKey::from_slice(&[seed; 32]).unwrap(),
            tunnel: TunnelKeys::from_secrets([seed; 32], [seed; 64]),
        }
    }

    fn entry(&self) -> Entry {
        Entry {
            dial: client::public(&self.dial),
            tunnel: self.tunnel.identity().x25519(),
            label: "desk".to_owned(),
        }
    }
}

fn authorize(dir: &Path, desktops: &[&Desktop]) {
    let mut authorized = Authorized::default();
    for desktop in desktops {
        authorized.insert(desktop.entry()).unwrap();
    }
    authorized.save(dir).unwrap();
}

struct Running {
    agent: Arc<Agent>,
    quic: SocketAddr,
    pin: Pin,
    _stop: watch::Sender<bool>,
}

async fn start(dir: &Path) -> Running {
    let (http, quic, pin) = relay().await;
    let (agent, stop) = start_on(dir, http).await;
    Running {
        agent,
        quic,
        pin,
        _stop: stop,
    }
}

/// An agent on `dir` listening on the relay at `http`, once it listens.
async fn start_on(dir: &Path, http: SocketAddr) -> (Arc<Agent>, watch::Sender<bool>) {
    let forward = ssh_server().await;
    let options = Options {
        forward,
        relays: vec![Relay {
            endpoint: Endpoint::parse(&format!("http://{http}")).unwrap(),
            token: Vec::new(),
        }],
        state_dir: dir.to_owned(),
    };
    let agent = Agent::load(options).unwrap();
    let (stop, stopped) = watch::channel(false);
    tokio::spawn(Arc::clone(&agent).run(stopped));
    until(&agent, |state| *state == RelayState::Listening).await;
    (agent, stop)
}

/// Waits, up to five seconds, for the agent's one relay to be in a state
/// `wanted` accepts.
async fn until(agent: &Agent, wanted: impl Fn(&RelayState) -> bool) {
    for _ in 0..100 {
        if agent.relay_states().values().any(&wanted) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never reached the state: {:?}", agent.relay_states());
}

/// A session's plaintext end, and the QUIC connection that carries it.
struct Session {
    io: tokio::io::DuplexStream,
    _connection: quinn::Connection,
}

/// Dials the agent as `desktop` and runs the tunnel.
async fn dial(running: &Running, desktop: &Desktop) -> sheprd_tunnel::Result<Session> {
    let connection = client::connect(running.quic, running.pin).await.unwrap();
    let route = running.agent.route();
    let (send, recv) = client::dial(&connection, route, &desktop.dial)
        .await
        .map_err(|_| sheprd_tunnel::Error::Closed)?;
    let identity = running.agent.keys.tunnel.identity();
    let identity = Identity::from_bytes(&identity.to_bytes()).unwrap();
    let (initiator, first) = Initiator::session(&desktop.tunnel, &identity, route.as_bytes())?;
    let tunnel = stream::connect(join(recv, send), initiator, &first, identity.x25519()).await?;
    Ok(Session {
        io: tunnel.io,
        _connection: connection,
    })
}

async fn greeting(io: &mut tokio::io::DuplexStream) -> Vec<u8> {
    let mut greeting = vec![0; 14];
    tokio::time::timeout(Duration::from_secs(5), io.read_exact(&mut greeting))
        .await
        .expect("no greeting")
        .unwrap();
    greeting
}

#[tokio::test(flavor = "multi_thread")]
async fn an_authorized_desktop_reaches_the_ssh_server_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(0x31);
    authorize(dir.path(), &[&desktop]);
    let running = start(dir.path()).await;

    let mut session = dial(&running, &desktop).await.unwrap();
    let io = &mut session.io;
    assert_eq!(greeting(io).await, b"SSH-2.0-test\r\n");
    io.write_all(b"ping").await.unwrap();
    let mut echo = [0; 4];
    io.read_exact(&mut echo).await.unwrap();
    assert_eq!(&echo, b"ping");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_desktop_not_authorized_hears_offline_at_the_relay() {
    let dir = tempfile::tempdir().unwrap();
    authorize(dir.path(), &[&Desktop::new(0x31)]);
    let running = start(dir.path()).await;
    let stranger = Desktop::new(0x77);
    let connection = client::connect(running.quic, running.pin).await.unwrap();
    assert!(matches!(
        client::dial(&connection, running.agent.route(), &stranger.dial).await,
        Err(client::Error::Refused(Status::Offline))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dial_key_with_another_desktops_tunnel_key_is_refused_in_the_tunnel() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(0x31);
    authorize(dir.path(), &[&desktop]);
    let running = start(dir.path()).await;
    // Someone holding the dial key but not the tunnel key gets nothing
    // past the relay.
    let impostor = Desktop {
        dial: desktop.dial.clone(),
        tunnel: TunnelKeys::from_secrets([0x99; 32], [0x99; 64]),
    };
    let dialed = tokio::time::timeout(Duration::from_secs(5), dial(&running, &impostor)).await;
    assert!(
        matches!(dialed, Ok(Err(_)) | Err(_)),
        "the impostor got through"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_desktop_ends_its_session() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::new(0x31);
    let other = Desktop::new(0x32);
    authorize(dir.path(), &[&desktop, &other]);
    let running = start(dir.path()).await;
    let mut session = dial(&running, &desktop).await.unwrap();
    let io = &mut session.io;
    greeting(io).await;

    authorize(dir.path(), &[&other]);
    running.agent.reload().unwrap();
    let mut rest = Vec::new();
    let ended = tokio::time::timeout(Duration::from_secs(5), io.read_to_end(&mut rest)).await;
    assert!(ended.is_ok(), "the session outlived its desktop");
    // The other desktop still gets through.
    let mut session = dial(&running, &other).await.unwrap();
    assert_eq!(greeting(&mut session.io).await, b"SSH-2.0-test\r\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_another_replaced_stops_reporting_listening() {
    let (http, _, _) = relay().await;
    let first_dir = tempfile::tempdir().unwrap();
    let (first, _first_stop) = start_on(first_dir.path(), http).await;
    // A copy of the same keys elsewhere: the relay gives it the route.
    let second_dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        first_dir.path().join(crate::keys::FILE),
        second_dir.path().join(crate::keys::FILE),
    )
    .unwrap();
    let (second, _second_stop) = start_on(second_dir.path(), http).await;
    assert_eq!(first.route(), second.route());
    let replaced = client::Error::Replaced.to_string();
    until(
        &first,
        |state| matches!(state, RelayState::Failed { error } if error.ends_with(&replaced)),
    )
    .await;
    // It waits the longest backoff before trying again, so the second
    // keeps the route rather than the two trading it.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(
        second
            .relay_states()
            .values()
            .all(|state| *state == RelayState::Listening)
    );
}
