/// Why a tunnel could not be set up or carried on.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("the tunnel's other side closed it")]
    Closed,
    #[error("a frame is longer than a Noise message may be")]
    FrameTooLong,
    #[error("the handshake did not hold: wrong keys, or a message changed on the way")]
    Handshake(#[source] snow::Error),
    #[error("an identity is not an X25519 key and an ML-KEM-768 key")]
    Identity,
    #[error("the other side's handshake message is not what this version sends")]
    Malformed,
    #[error("could not draw random bytes")]
    Random(#[source] getrandom::Error),
    #[error("a record did not decrypt: changed, replayed, or out of order")]
    Record(#[source] snow::Error),
    #[error("the dialer's key is not one this side accepts")]
    Refused,
    // keep-sorted end
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
