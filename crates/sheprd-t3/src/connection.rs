//! A blocking WebSocket to a T3 Code server, speaking Effect RPC frames.
//!
//! It runs on the caller's thread, as Sheprd's other socket workers do; no
//! async runtime. Every stream chunk is acknowledged as soon as it is read,
//! since the caller handles a chunk before reading the next frame. Messages
//! are bounded by [`MAX_MESSAGE`], and an unanswered read gives up after the
//! stream's read timeout.

use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs as _},
    time::Duration,
};

use serde_json::Value;
use tungstenite::{Message, WebSocket, handshake::HandshakeError, protocol::WebSocketConfig};
use url::Url;

use crate::{
    Error, Result,
    frame::{ClientFrame, Exit, ServerFrame},
};

/// The largest message this client reads: a thread's snapshot carries its
/// whole projection, which a long thread makes large.
pub const MAX_MESSAGE: usize = 32 * 1024 * 1024;

/// How long a connection attempt or a quiet read waits.
pub const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Connection<S> {
    next: u64,
    socket: WebSocket<S>,
}

impl Connection<TcpStream> {
    /// Opens the socket at `url`, a `ws://` address from
    /// [`crate::http::Server::socket`].
    pub fn open(url: &Url) -> Result<Self> {
        if url.scheme() != "ws" {
            // TLS comes with remote servers; a local server listens in plain.
            return Err(Error::Scheme(url.scheme().to_owned()));
        }
        let host = url
            .host_str()
            .ok_or(Error::Url(url::ParseError::EmptyHost))?;
        let port = url.port_or_known_default().unwrap_or(80);
        let addresses = (host, port).to_socket_addrs().map_err(Error::Connect)?;
        let stream = connect_any(addresses, |address| {
            TcpStream::connect_timeout(address, TIMEOUT)
        })?;
        stream
            .set_read_timeout(Some(TIMEOUT))
            .map_err(Error::Connect)?;
        Self::handshake(url, stream)
    }
}

/// The first of `addresses` that `connect` reaches, in resolver order.
///
/// A name such as `localhost` resolves to both `::1` and `127.0.0.1`, and a
/// local server often listens on only one, so every address is tried before
/// giving up with the last failure.
fn connect_any<A, T>(
    addresses: impl IntoIterator<Item = A>,
    mut connect: impl FnMut(&A) -> std::io::Result<T>,
) -> Result<T> {
    let mut failure = None;
    for address in addresses {
        match connect(&address) {
            Ok(stream) => return Ok(stream),
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.map_or(Error::NoServer, Error::Connect))
}

impl<S: Read + Write> Connection<S> {
    /// The client handshake for `url` over `stream`.
    pub fn handshake(url: &Url, stream: S) -> Result<Self> {
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE))
            .max_frame_size(Some(MAX_MESSAGE));
        match tungstenite::client::client_with_config(url.as_str(), stream, Some(config)) {
            Ok((socket, _)) => Ok(Self { next: 0, socket }),
            Err(HandshakeError::Failure(error)) => Err(error.into()),
            Err(HandshakeError::Interrupted(_)) => Err(Error::Closed),
        }
    }

    /// Sends a call or a stream request for `tag`; returns its request id.
    pub fn request(&mut self, tag: &str, payload: Value) -> Result<String> {
        self.next += 1;
        let id = self.next.to_string();
        self.send(&ClientFrame::Request {
            id: id.clone(),
            tag: tag.to_owned(),
            payload,
            headers: [],
        })?;
        Ok(id)
    }

    /// The next frame from the server, acknowledging a stream chunk at once.
    pub fn next_frame(&mut self) -> Result<ServerFrame> {
        loop {
            let text = match self.socket.read()? {
                Message::Text(text) => text,
                Message::Close(_) => return Err(Error::Closed),
                // Pings are answered by the socket itself on the next write.
                Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {
                    continue;
                }
            };
            let frame: ServerFrame = serde_json::from_str(text.as_str())?;
            if let ServerFrame::Chunk { request_id, .. } = &frame {
                self.send(&ClientFrame::Ack {
                    request_id: request_id.clone(),
                })?;
            }
            return Ok(frame);
        }
    }

    /// Calls `tag` and waits for its answer, skipping frames of other
    /// requests.
    pub fn call(&mut self, tag: &str, payload: Value) -> Result<Value> {
        let id = self.request(tag, payload)?;
        loop {
            if let ServerFrame::Exit { request_id, exit } = self.next_frame()?
                && request_id == id
            {
                return match exit {
                    Exit::Success { value } => Ok(value),
                    failure => Err(Error::Call {
                        cause: failure.cause().unwrap_or_default(),
                        tag: tag.to_owned(),
                    }),
                };
            }
        }
    }

    /// Checks the server is still there after a quiet spell
    /// ([`Error::Idle`]); its `Pong` comes as a frame.
    pub fn ping(&mut self) -> Result<()> {
        self.send(&ClientFrame::Ping)
    }

    /// Asks the server to stop the stream `request_id`.
    pub fn interrupt(&mut self, request_id: &str) -> Result<()> {
        self.send(&ClientFrame::Interrupt {
            request_id: request_id.to_owned(),
        })
    }

    /// Closes the socket, waiting briefly for the server's close.
    pub fn close(mut self) {
        let _ = self.socket.close(None);
        while self.socket.read().is_ok() {}
    }

    fn send(&mut self, frame: &ClientFrame) -> Result<()> {
        let text = serde_json::to_string(frame)?;
        self.socket.send(Message::Text(text.into()))?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
