//! Reaching a relay over QUIC, as a listener (a computer) or a dialer: the
//! client side of [`crate::wire`]. Each side proves its P-256 key over the
//! connection's TLS exporter, so a proof cannot be replayed onto another
//! connection. What crosses a session is the end-to-end tunnel's
//! (`sheprd-tunnel`); this module only carries it.

use crate::{
    RouteId,
    pin::{self, Pin, SERVER_NAME, client_transport},
    wire::{self, EXPORTER_LABEL, EXPORTER_LEN, INCOMING, KEY_LEN, Key, Message, Request, Status},
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream};
use std::{net::SocketAddr, sync::Arc};

mod reach;
pub use reach::{first_reachable, reach};

/// Why reaching a relay, or a step on it, failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("could not open a UDP socket for QUIC")]
    Bind(#[source] std::io::Error),
    #[error("could not start a QUIC connection")]
    Connect(#[from] quinn::ConnectError),
    #[error("the QUIC connection failed or closed")]
    Connection(#[from] quinn::ConnectionError),
    #[error("the connection offers no TLS exporter to prove a key over")]
    Exporter,
    #[error("the relay's /v1/info could not be read")]
    Info(#[from] Box<ureq::Error>),
    #[error("the relay's /v1/info is not what a relay answers")]
    InfoFormat,
    #[error("the relay ended this listener")]
    ListenEnded,
    #[error("the relay sent something that is not its protocol")]
    Malformed,
    #[error("the relay offers no QUIC")]
    NoQuic,
    #[error("could not trust the relay's certificate")]
    Pin(#[from] pin::Error),
    #[error("could not read from the relay")]
    Read(#[from] quinn::ReadExactError),
    #[error("the relay answered {0:?}")]
    Refused(Status),
    #[error("another listener with the same key took this route")]
    Replaced,
    #[error("the relay's host name did not resolve")]
    Resolve(#[source] std::io::Error),
    #[error("could not write to the relay")]
    Write(#[from] quinn::WriteError),
    // keep-sorted end
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// `key`'s public half as the wire carries it: SEC1, compressed.
#[must_use]
pub fn public(key: &SigningKey) -> Key {
    let mut public = [0; KEY_LEN];
    public.copy_from_slice(key.verifying_key().to_sec1_point(true).as_bytes());
    public
}

/// The route `key` listens on.
#[must_use]
pub fn route(key: &SigningKey) -> RouteId {
    RouteId::of(key.verifying_key())
}

fn sign(key: &SigningKey, message: &[u8]) -> [u8; 64] {
    let signature: Signature = key.sign(message);
    signature.to_bytes().into()
}

/// Connects to the relay at `address`, trusting only the certificate with
/// `pin`.
pub async fn connect(address: SocketAddr, pin: Pin) -> Result<Connection> {
    let local: SocketAddr = if address.is_ipv4() {
        SocketAddr::from(([0, 0, 0, 0], 0))
    } else {
        SocketAddr::from(([0u16; 8], 0))
    };
    let mut endpoint = Endpoint::client(local).map_err(Error::Bind)?;
    let mut config = ClientConfig::new(Arc::new(pin::client_crypto(pin)?));
    config.transport_config(Arc::new(client_transport()));
    endpoint.set_default_client_config(config);
    Ok(endpoint.connect(address, SERVER_NAME)?.await?)
}

fn exported(connection: &Connection) -> Result<[u8; EXPORTER_LEN]> {
    let mut exported = [0; EXPORTER_LEN];
    connection
        .export_keying_material(&mut exported, EXPORTER_LABEL, b"")
        .map_err(|_| Error::Exporter)?;
    Ok(exported)
}

async fn status(recv: &mut RecvStream) -> Result<Status> {
    let mut byte = [0; 1];
    recv.read_exact(&mut byte).await?;
    Status::from_byte(byte[0]).ok_or(Error::Malformed)
}

async fn expect_ok(recv: &mut RecvStream) -> Result<()> {
    match status(recv).await? {
        Status::Ok => Ok(()),
        other => Err(Error::Refused(other)),
    }
}

/// A computer listening on its route, until dropped.
#[derive(Debug)]
pub struct Listener {
    connection: Connection,
    /// The control stream's answers. Behind a lock only so [`Listener::closed`]
    /// can watch it for a reset from `&self` beside [`Listener::next_stream`];
    /// [`Listener::message`] reaches it through `&mut self` without waiting.
    recv: tokio::sync::Mutex<RecvStream>,
    route: RouteId,
    send: SendStream,
}

/// One dial the relay handed to a listener: the dialer's key, as the relay
/// tells it (authorize on the tunnel's identity, never this), and the
/// session's two halves.
#[derive(Debug)]
pub struct Incoming {
    pub key: Key,
    pub recv: RecvStream,
    pub send: SendStream,
}

/// A stream the relay opened for a dial, its header not read yet.
#[derive(Debug)]
pub struct Pending {
    recv: RecvStream,
    send: SendStream,
}

impl Pending {
    /// Reads the header the relay puts first: which key dialed.
    pub async fn incoming(mut self) -> Result<Incoming> {
        let mut header = [0; 1 + KEY_LEN];
        self.recv.read_exact(&mut header).await?;
        let [kind, key @ ..] = header;
        if kind != INCOMING {
            return Err(Error::Malformed);
        }
        Ok(Incoming {
            key,
            recv: self.recv,
            send: self.send,
        })
    }
}

impl Listener {
    /// Listens on `key`'s route over `connection`, with the relay's listen
    /// token when it asks for one (empty otherwise). No one may dial until
    /// [`Listener::set_dial_keys`] names them.
    pub async fn listen(connection: Connection, key: &SigningKey, token: &[u8]) -> Result<Self> {
        let request = Request::Listen {
            key: public(key),
            signature: sign(key, &wire::listen_signed(&exported(&connection)?)),
            token: token.to_vec(),
        };
        let (mut send, mut recv) = connection.open_bi().await?;
        send.write_all(&request.encode().map_err(|_| Error::Malformed)?)
            .await?;
        expect_ok(&mut recv).await?;
        Ok(Self {
            connection,
            recv: tokio::sync::Mutex::new(recv),
            route: route(key),
            send,
        })
    }

    /// The route this listener holds.
    #[must_use]
    pub fn route(&self) -> RouteId {
        self.route
    }

    async fn message(&mut self, message: Message) -> Result<()> {
        let bytes = message.encode().map_err(|_| Error::Malformed)?;
        self.send.write_all(&bytes).await?;
        expect_ok(self.recv.get_mut()).await
    }

    /// Replaces the keys that may dial this computer; sessions of keys left
    /// out end at once.
    pub async fn set_dial_keys(&mut self, keys: Vec<Key>) -> Result<()> {
        self.message(Message::DialKeys(keys)).await
    }

    /// Allows `key` one dial within `seconds`, for pairing.
    pub async fn add_pairing_key(&mut self, key: Key, seconds: u16) -> Result<()> {
        self.message(Message::PairingKey { key, seconds }).await
    }

    /// The next dial: a stream the relay opened on this connection, its
    /// header read. Not cancel-safe: dropped while the header is arriving,
    /// it drops that dial. A listener that waits on other events beside it
    /// takes [`Listener::next_stream`] instead.
    pub async fn accept(&self) -> Result<Incoming> {
        self.next_stream().await?.incoming().await
    }

    /// The next stream the relay opened, before its header is read.
    /// Cancel-safe, so it can wait beside timers and other events; read the
    /// header with [`Pending::incoming`], away from them.
    pub async fn next_stream(&self) -> Result<Pending> {
        let (send, recv) = self.connection.accept_bi().await?;
        Ok(Pending { recv, send })
    }

    /// The connection's smoothed round-trip time to the relay, which QUIC
    /// keeps current from its keep-alives: how near the relay is.
    #[must_use]
    pub fn rtt(&self) -> std::time::Duration {
        self.connection.rtt()
    }

    /// Resolves when the relay drops this listener or the connection ends:
    /// [`Error::Replaced`] when another listener proved the same key and
    /// took the route (the relay resets this one's control stream but keeps
    /// the connection, which may carry other work), [`Error::ListenEnded`]
    /// when the control stream ends otherwise. Cancel-safe.
    pub async fn closed(&self) -> Error {
        let mut recv = self.recv.lock().await;
        tokio::select! {
            error = self.connection.closed() => Error::Connection(error),
            reset = recv.received_reset() => match reset {
                Ok(Some(code)) if code == quinn::VarInt::from_u32(wire::code::REPLACED) => {
                    Error::Replaced
                }
                Err(quinn::ResetError::ConnectionLost(error)) => Error::Connection(error),
                Ok(_) | Err(_) => Error::ListenEnded,
            },
        }
    }
}

/// Dials `route` as `key` over `connection`: the session's two halves once
/// the listener accepted.
pub async fn dial(
    connection: &Connection,
    route: RouteId,
    key: &SigningKey,
) -> Result<(SendStream, RecvStream)> {
    let request = Request::Dial {
        key: public(key),
        route,
        signature: sign(key, &wire::dial_signed(&exported(connection)?, &route)),
    };
    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(&request.encode().map_err(|_| Error::Malformed)?)
        .await?;
    expect_ok(&mut recv).await?;
    Ok((send, recv))
}
