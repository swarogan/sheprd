//! The tunnel over a Tokio stream: the handshake, then a task that carries
//! records between the relay stream and a plain in-memory pipe, whose
//! other end the caller reads and writes like any socket (hyper serves the
//! daemon's API over it unchanged).

use crate::{
    Error, Initiator, Keys, Responder, Result, Transport,
    frame::{self, Decoder},
    record::{MAX_DATA, Opened},
};
use std::time::Instant;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, duplex},
    task::JoinHandle,
};

/// How much plaintext the pipe holds each way before the pump waits.
const PIPE: usize = 64 * 1024;

/// A finished tunnel: the plaintext end, and the task carrying it.
#[derive(Debug)]
pub struct Tunnel {
    /// The handshake's hash: the pairing code comes from it.
    pub hash: [u8; 32],
    /// Read and write plaintext here. Dropping it closes the tunnel.
    pub io: DuplexStream,
    /// The dialer's static X25519 key (the listener's, on the dialer's
    /// side, is the one it pinned).
    pub remote: [u8; 32],
    /// Ends when either side closes or the stream breaks.
    pub task: JoinHandle<Result<()>>,
}

/// Dials over `stream`: sends `first` (from [`Initiator::session`] or
/// [`Initiator::pairing`]) and finishes on the listener's answer.
pub async fn connect<S>(
    mut stream: S,
    initiator: Initiator,
    first: &[u8],
    listener: [u8; 32],
) -> Result<Tunnel>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    write_frame(&mut stream, first).await?;
    let mut decoder = Decoder::default();
    let answer = read_frame(&mut stream, &mut decoder).await?;
    let (transport, hash) = initiator.finish(&answer)?;
    Ok(start(stream, decoder, transport, hash, listener))
}

/// Listens over `stream`: reads the dialer's first message, asks
/// `authorize` whether its static key may connect, and answers only then.
/// A refused dialer hears nothing.
pub async fn accept<S>(
    mut stream: S,
    local: &Keys,
    responder: Responder,
    authorize: impl FnOnce(&[u8; 32]) -> bool,
) -> Result<Tunnel>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut decoder = Decoder::default();
    let first = read_frame(&mut stream, &mut decoder).await?;
    let pending = responder.read(local, &first)?;
    let remote = pending.remote();
    if !authorize(&remote) {
        return Err(Error::Refused);
    }
    let (answer, transport, hash) = pending.accept()?;
    write_frame(&mut stream, &answer).await?;
    Ok(start(stream, decoder, transport, hash, remote))
}

fn start<S>(
    stream: S,
    decoder: Decoder,
    transport: Transport,
    hash: [u8; 32],
    remote: [u8; 32],
) -> Tunnel
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (io, inner) = duplex(PIPE);
    let task = tokio::spawn(pump(stream, decoder, transport, inner));
    Tunnel {
        hash,
        io,
        remote,
        task,
    }
}

async fn write_frame<S: AsyncWrite + Unpin>(stream: &mut S, message: &[u8]) -> Result<()> {
    let framed = frame::encode(message)?;
    stream.write_all(&framed).await.map_err(|_| Error::Closed)?;
    stream.flush().await.map_err(|_| Error::Closed)
}

/// Reads one frame, never more than the frame needs.
async fn read_frame<S: AsyncRead + Unpin>(
    stream: &mut S,
    decoder: &mut Decoder,
) -> Result<Vec<u8>> {
    loop {
        if let Some(frame) = decoder.next_frame() {
            return Ok(frame);
        }
        let mut chunk = vec![0; decoder.wanted()];
        let read = stream.read(&mut chunk).await.map_err(|_| Error::Closed)?;
        if read == 0 {
            return Err(Error::Closed);
        }
        decoder.push(&chunk[..read]);
    }
}

/// Bytes on their way out of the pump, kept until the other side takes
/// them: a write that waits never stops the pump reading the other way.
#[derive(Default)]
struct Pending {
    bytes: Vec<u8>,
    sent: usize,
}

impl Pending {
    fn is_empty(&self) -> bool {
        self.sent == self.bytes.len()
    }

    fn rest(&self) -> &[u8] {
        &self.bytes[self.sent..]
    }

    fn set(&mut self, bytes: Vec<u8>) {
        self.bytes = bytes;
        self.sent = 0;
    }

    /// Counts `written` bytes as taken; none taken means the other side
    /// is gone.
    fn advance(&mut self, written: usize) -> Result<()> {
        if written == 0 {
            return Err(Error::Closed);
        }
        self.sent += written;
        Ok(())
    }
}

/// Carries records until either side closes: plaintext from the pipe is
/// sealed and framed onto the stream; frames from the stream are opened
/// into the pipe.
///
/// Each direction holds at most one record and moves on its own. Were a
/// write awaited to its end, two peers sending at once would each fill
/// the stream and wait for the other to read, which neither then does.
async fn pump<S>(
    stream: S,
    mut decoder: Decoder,
    mut transport: Transport,
    pipe: DuplexStream,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut from_peer, mut to_peer) = tokio::io::split(stream);
    let (mut from_local, mut to_local) = tokio::io::split(pipe);
    let mut plain = vec![0; MAX_DATA];
    let mut chunk = vec![0; frame::MAX_FRAME];
    let mut inbound = Pending::default();
    let mut outbound = Pending::default();
    let mut local_open = true;
    let mut unflushed = false;
    loop {
        // Frames already buffered go first, so a burst is not left behind.
        // One at a time: the peer's close is seen only after its data left.
        if inbound.is_empty()
            && let Some(frame) = decoder.next_frame()
        {
            match transport.open(&frame)? {
                Opened::Data(data) => inbound.set(data),
                Opened::Closed => {
                    let _ = to_local.shutdown().await;
                    return Ok(());
                }
            }
            continue;
        }
        let wanted = decoder.wanted().clamp(4_096, frame::MAX_FRAME);
        let flushing = unflushed && outbound.is_empty();
        tokio::select! {
            read = from_peer.read(&mut chunk[..wanted]), if inbound.is_empty() => {
                let read = read.map_err(|_| Error::Closed)?;
                if read == 0 {
                    // Cut without a close record: the relay or network.
                    return Err(Error::Closed);
                }
                decoder.push(&chunk[..read]);
            }
            written = to_local.write(inbound.rest()), if !inbound.is_empty() => {
                inbound.advance(written.map_err(|_| Error::Closed)?)?;
            }
            read = from_local.read(&mut plain), if local_open && outbound.is_empty() => {
                let read = read.map_err(|_| Error::Closed)?;
                let record = if read == 0 {
                    local_open = false;
                    transport.seal_close()?
                } else {
                    transport.seal(&plain[..read], Instant::now())?
                };
                outbound.set(frame::encode(&record)?);
            }
            // One branch for the stream's write half: the record, then
            // its flush.
            written = async {
                if flushing {
                    to_peer.flush().await.map(|()| 0)
                } else {
                    to_peer.write(outbound.rest()).await
                }
            }, if flushing || !outbound.is_empty() => {
                let written = written.map_err(|_| Error::Closed)?;
                if flushing {
                    unflushed = false;
                } else {
                    outbound.advance(written)?;
                    unflushed = outbound.is_empty();
                }
            }
        }
    }
}
