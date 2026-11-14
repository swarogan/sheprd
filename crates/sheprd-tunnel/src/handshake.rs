//! The handshake: `Noise_IKpsk2_25519_ChaChaPoly_SHA256`, two messages.
//!
//! ```text
//! <- s
//! ...
//! -> e, es, s, ss        payload: version ‖ ML-KEM ciphertext (sessions)
//! <- e, ee, se, psk      payload: version
//! ```
//!
//! **Sessions.** The dialer knows the listener's [`Identity`] from pairing.
//! It encapsulates to the listener's ML-KEM-768 key and sends the
//! ciphertext in the first message, already encrypted under `es` and `ss`;
//! the shared secret is the PSK, mixed in at the end of the second
//! message. Either X25519 or ML-KEM-768 holding keeps the session closed.
//!
//! **Pairing.** The dialer knows only the listener's X25519 key and
//! identity fingerprint, from the QR. The PSK comes from the QR's secret,
//! so only someone who saw the QR completes it, and a recording stays
//! closed even to a quantum computer. The listener's full identity, and
//! the dialer's, cross inside the tunnel afterwards.
//!
//! The prologue binds the version, the mode, the relay route, and the
//! listener's identity fingerprint, so a stream cannot be replayed onto
//! another route or listener. The listener learns the dialer's static key
//! from the first message and decides, before answering, whether it is
//! one it accepts ([`Pending`]).

use crate::{
    Error, PROTOCOL, Result, VERSION,
    keys::{Identity, Keys, MLKEM_CIPHERTEXT_LEN, X25519_LEN},
    record::Transport,
};
use hkdf::Hkdf;
use sha2::Sha256;
use snow::{Builder, HandshakeState};

/// What every prologue starts with.
const PROLOGUE: &[u8] = b"sheprd-tunnel/v1\0";
/// Where the PSK goes: `psk2`, at the end of the second message.
const PSK_AT: u8 = 2;
/// The salt that turns a pairing secret into the handshake's PSK.
const PAIRING_SALT: &[u8] = b"sheprd-tunnel/pairing/v1";
/// The salt that turns a pairing handshake into the code both screens show.
const CODE_SALT: &[u8] = b"sheprd-tunnel/pairing-code/v1";
/// A relay route's length: the SHA-256 of the listener's relay key.
pub const ROUTE_LEN: usize = 32;
/// A pairing secret's length, as the QR carries it.
pub const SECRET_LEN: usize = 32;

/// Which handshake: a paired device's session, or pairing a new one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Session,
    Pairing,
}

impl Mode {
    const fn byte(self) -> u8 {
        match self {
            Self::Session => 1,
            Self::Pairing => 2,
        }
    }
}

/// The prologue both sides bind: version, mode, route, and the listener.
#[must_use]
pub fn prologue(mode: Mode, route: &[u8; ROUTE_LEN], listener: &[u8; 32]) -> Vec<u8> {
    [PROLOGUE, &[VERSION, mode.byte()], route, listener].concat()
}

/// The pairing handshake's PSK, from the QR's secret.
#[must_use]
pub fn pairing_psk(secret: &[u8; SECRET_LEN]) -> [u8; 32] {
    let mut psk = [0; 32];
    // A 32-byte output is always within HKDF-SHA256's limit.
    let _ = Hkdf::<Sha256>::new(Some(PAIRING_SALT), secret).expand(b"psk", &mut psk);
    psk
}

/// The six-digit code both screens show when pairing, from the finished
/// handshake's hash: the same on both sides only if no one sat between
/// them.
#[must_use]
pub fn pairing_code(handshake_hash: &[u8]) -> u32 {
    let mut bytes = [0; 4];
    let _ = Hkdf::<Sha256>::new(Some(CODE_SALT), handshake_hash).expand(b"code", &mut bytes);
    u32::from_be_bytes(bytes) % 1_000_000
}

/// What a test fixes that a real handshake draws at random.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Fixed {
    pub ephemeral: Option<[u8; 32]>,
    pub encapsulation: Option<[u8; 32]>,
}

fn builder<'a>(
    local: &'a [u8; X25519_LEN],
    prologue: &'a [u8],
    fixed: &'a Fixed,
) -> Result<Builder<'a>> {
    let params = PROTOCOL.parse().map_err(Error::Handshake)?;
    let mut builder = Builder::new(params)
        .local_private_key(local)
        .map_err(Error::Handshake)?
        .prologue(prologue)
        .map_err(Error::Handshake)?;
    if let Some(ephemeral) = &fixed.ephemeral {
        builder = builder.fixed_ephemeral_key_for_testing_only(ephemeral);
    }
    Ok(builder)
}

/// The dialer's side, after its first message.
#[derive(Debug)]
pub struct Initiator {
    state: HandshakeState,
}

impl Initiator {
    /// Dials a paired listener: its pinned `listener` identity, on `route`.
    /// Returns the first message to send.
    pub fn session(
        local: &Keys,
        listener: &Identity,
        route: &[u8; ROUTE_LEN],
    ) -> Result<(Self, Vec<u8>)> {
        let mut randomness = [0; 32];
        getrandom::fill(&mut randomness).map_err(Error::Random)?;
        let fixed = Fixed {
            encapsulation: Some(randomness),
            ..Fixed::default()
        };
        Self::session_fixed(local, listener, route, &fixed)
    }

    #[doc(hidden)]
    pub fn session_fixed(
        local: &Keys,
        listener: &Identity,
        route: &[u8; ROUTE_LEN],
        fixed: &Fixed,
    ) -> Result<(Self, Vec<u8>)> {
        let randomness = fixed.encapsulation.ok_or(Error::Malformed)?;
        let (ciphertext, shared) = listener.encapsulate(&randomness)?;
        let prologue = prologue(Mode::Session, route, &listener.fingerprint());
        let secret = local.x25519_secret();
        let remote = listener.x25519();
        let mut state = builder(&secret, &prologue, fixed)?
            .remote_public_key(&remote)
            .map_err(Error::Handshake)?
            .build_initiator()
            .map_err(Error::Handshake)?;
        // Known before the second message, which mixes it in.
        state
            .set_psk(usize::from(PSK_AT), &shared)
            .map_err(Error::Handshake)?;
        let payload = [&[VERSION][..], &ciphertext].concat();
        let message = write(&mut state, &payload)?;
        Ok((Self { state }, message))
    }

    /// Pairs with a listener known only by the QR: its X25519 key, its
    /// identity's fingerprint, its route, and the pairing secret.
    pub fn pairing(
        local: &Keys,
        listener_x25519: &[u8; X25519_LEN],
        listener_fingerprint: &[u8; 32],
        route: &[u8; ROUTE_LEN],
        secret: &[u8; SECRET_LEN],
    ) -> Result<(Self, Vec<u8>)> {
        Self::pairing_fixed(
            local,
            (listener_x25519, listener_fingerprint),
            route,
            secret,
            &Fixed::default(),
        )
    }

    #[doc(hidden)]
    pub fn pairing_fixed(
        local: &Keys,
        (listener_x25519, listener_fingerprint): (&[u8; X25519_LEN], &[u8; 32]),
        route: &[u8; ROUTE_LEN],
        secret: &[u8; SECRET_LEN],
        fixed: &Fixed,
    ) -> Result<(Self, Vec<u8>)> {
        let prologue = prologue(Mode::Pairing, route, listener_fingerprint);
        let psk = pairing_psk(secret);
        let local_secret = local.x25519_secret();
        let mut state = builder(&local_secret, &prologue, fixed)?
            .remote_public_key(listener_x25519)
            .map_err(Error::Handshake)?
            .psk(PSK_AT, &psk)
            .map_err(Error::Handshake)?
            .build_initiator()
            .map_err(Error::Handshake)?;
        let message = write(&mut state, &[VERSION])?;
        Ok((Self { state }, message))
    }

    /// Reads the listener's answer: the tunnel, and the handshake's hash
    /// (for [`pairing_code`]).
    pub fn finish(mut self, answer: &[u8]) -> Result<(Transport, [u8; 32])> {
        let payload = read(&mut self.state, answer)?;
        if payload != [VERSION] {
            return Err(Error::Malformed);
        }
        transport(self.state)
    }
}

/// The listener's side, before the first message.
#[derive(Debug)]
pub struct Responder {
    mode: Mode,
    state: HandshakeState,
}

impl Responder {
    /// Takes a paired device's session on `route`.
    pub fn session(local: &Keys, route: &[u8; ROUTE_LEN]) -> Result<Self> {
        Self::new(local, Mode::Session, route, None, &Fixed::default())
    }

    /// Takes a pairing dial on `route`, for the QR showing `secret`.
    pub fn pairing(
        local: &Keys,
        route: &[u8; ROUTE_LEN],
        secret: &[u8; SECRET_LEN],
    ) -> Result<Self> {
        Self::new(local, Mode::Pairing, route, Some(secret), &Fixed::default())
    }

    #[doc(hidden)]
    pub fn new(
        local: &Keys,
        mode: Mode,
        route: &[u8; ROUTE_LEN],
        secret: Option<&[u8; SECRET_LEN]>,
        fixed: &Fixed,
    ) -> Result<Self> {
        let prologue = prologue(mode, route, &local.identity().fingerprint());
        let local_secret = local.x25519_secret();
        let psk = secret.map(pairing_psk);
        let mut builder = builder(&local_secret, &prologue, fixed)?;
        if let Some(psk) = &psk {
            builder = builder.psk(PSK_AT, psk).map_err(Error::Handshake)?;
        }
        let state = builder.build_responder().map_err(Error::Handshake)?;
        Ok(Self { mode, state })
    }

    /// Reads the dialer's first message. `local` must be the keys this
    /// responder was made with: a session's PSK is in the ML-KEM
    /// ciphertext only they open.
    pub fn read(mut self, local: &Keys, first: &[u8]) -> Result<Pending> {
        let payload = read(&mut self.state, first)?;
        let (version, rest) = payload.split_first().ok_or(Error::Malformed)?;
        if *version != VERSION {
            return Err(Error::Malformed);
        }
        match self.mode {
            Mode::Session => {
                if rest.len() != MLKEM_CIPHERTEXT_LEN {
                    return Err(Error::Malformed);
                }
                let shared = local.decapsulate(rest)?;
                self.state
                    .set_psk(usize::from(PSK_AT), &shared)
                    .map_err(Error::Handshake)?;
            }
            Mode::Pairing if !rest.is_empty() => return Err(Error::Malformed),
            Mode::Pairing => {}
        }
        let mut remote = [0; X25519_LEN];
        let given = self.state.get_remote_static().ok_or(Error::Malformed)?;
        if given.len() != X25519_LEN {
            return Err(Error::Malformed);
        }
        remote.copy_from_slice(given);
        Ok(Pending {
            remote,
            state: self.state,
        })
    }
}

/// A dial the listener has read but not answered: it checks [`Pending::remote`]
/// against its paired devices, then [`Pending::accept`]s, or drops it, which
/// sends nothing and so reveals nothing.
#[derive(Debug)]
pub struct Pending {
    remote: [u8; X25519_LEN],
    state: HandshakeState,
}

impl Pending {
    /// The dialer's static X25519 key, proven by the first message.
    #[must_use]
    pub fn remote(&self) -> [u8; X25519_LEN] {
        self.remote
    }

    /// Answers: the second message to send, the tunnel, and the
    /// handshake's hash (for [`pairing_code`]).
    pub fn accept(mut self) -> Result<(Vec<u8>, Transport, [u8; 32])> {
        let message = write(&mut self.state, &[VERSION])?;
        let (transport, hash) = transport(self.state)?;
        Ok((message, transport, hash))
    }
}

fn write(state: &mut HandshakeState, payload: &[u8]) -> Result<Vec<u8>> {
    let mut message = vec![0; crate::MAX_MESSAGE];
    let length = state
        .write_message(payload, &mut message)
        .map_err(Error::Handshake)?;
    message.truncate(length);
    Ok(message)
}

fn read(state: &mut HandshakeState, message: &[u8]) -> Result<Vec<u8>> {
    let mut payload = vec![0; crate::MAX_MESSAGE];
    let length = state
        .read_message(message, &mut payload)
        .map_err(Error::Handshake)?;
    payload.truncate(length);
    Ok(payload)
}

fn transport(state: HandshakeState) -> Result<(Transport, [u8; 32])> {
    let mut hash = [0; 32];
    hash.copy_from_slice(state.get_handshake_hash());
    let state = state.into_transport_mode().map_err(Error::Handshake)?;
    Ok((Transport::new(state), hash))
}
