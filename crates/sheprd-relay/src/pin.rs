//! Trusting a relay's QUIC certificate by its pin: the SHA-256 the signed
//! relay list carries, or `/v1/info` reported. Shared by the relay's tests
//! and the [`crate::client`].

use crate::wire::ALPN;
use quinn::{
    TransportConfig, VarInt,
    crypto::rustls::{NoInitialCipherSuite, QuicClientConfig},
};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};

/// The name in the relay's certificate. Clients check the pin, not names.
pub const SERVER_NAME: &str = "sheprd-relay";

/// A certificate's SHA-256, as clients pin it.
pub type Pin = [u8; 32];

/// A connection with nothing in flight and no keepalive is dropped after
/// this. Clients send QUIC keepalives more often.
pub(crate) const IDLE: Duration = Duration::from_secs(60);
/// Streams one connection may have open at once: a phone's sessions to a
/// few computers, or a computer's listen stream and its sessions.
pub(crate) const MAX_STREAMS: u32 = 64;
/// How much one stream, and one connection, of a client may have in
/// flight. quinn's defaults suit a single stream on a short path; a relay
/// hop carries bulk transfers over long ones, where a small window caps
/// throughput at window ÷ round trip. The relay's own windows are sized
/// to its sessions instead (`quic::window`).
const CLIENT_STREAM_WINDOW: u32 = 8 * 1024 * 1024;
const CLIENT_CONNECTION_WINDOW: u32 = 32 * 1024 * 1024;

/// The transport settings clients should use: keepalives under the relay's
/// idle timeout, so NAT mappings and the connection stay open.
#[must_use]
pub fn client_transport() -> TransportConfig {
    let mut transport = TransportConfig::default();
    transport
        .keep_alive_interval(Some(Duration::from_secs(15)))
        .max_idle_timeout(IDLE.try_into().ok())
        .max_concurrent_uni_streams(VarInt::from_u32(0))
        .max_concurrent_bidi_streams(VarInt::from_u32(MAX_STREAMS))
        .stream_receive_window(VarInt::from_u32(CLIENT_STREAM_WINDOW))
        .receive_window(VarInt::from_u32(CLIENT_CONNECTION_WINDOW))
        .send_window(u64::from(CLIENT_CONNECTION_WINDOW));
    transport
}

/// Why a pinned client configuration could not be made.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("the TLS setup has no cipher suite QUIC can start with")]
    Cipher(#[from] NoInitialCipherSuite),
    #[error("could not set up TLS")]
    Tls(#[from] rustls::Error),
    // keep-sorted end
}

pub(crate) fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A client config that trusts only the certificate with `pin`.
pub fn client_crypto(pin: Pin) -> Result<QuicClientConfig, Error> {
    let provider = provider();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { pin, provider }))
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN.to_vec()];
    Ok(QuicClientConfig::try_from(config)?)
}

/// Accepts exactly the pinned certificate, and still checks that the
/// handshake was signed by its key.
#[derive(Debug)]
struct Pinned {
    pin: Pin,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let seen: Pin = Sha256::digest(end_entity).into();
        if seen == self.pin {
            return Ok(ServerCertVerified::assertion());
        }
        Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::ApplicationVerificationFailure,
        ))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
