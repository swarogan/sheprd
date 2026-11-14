//! The whole path, as the daemon and the phone run it: a computer listens
//! through [`crate::client`], a phone dials through a real relay, and both
//! run the end-to-end tunnel over the session. The relay carries only
//! ciphertext.

use super::*;
use crate::client::{self, Listener, public};
use sheprd_tunnel::{Initiator, Keys, Responder, stream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, join};

#[tokio::test]
async fn a_paired_phone_reaches_its_computer_through_the_relay_end_to_end() {
    let relay = start_both(&options(Limits::default())).await;
    let computer_relay_key = signing_key(170);
    let phone_dial_key = signing_key(171);
    let computer_tunnel = Keys::from_secrets([0x31; 32], [0x32; 64]);
    let phone_tunnel = Keys::from_secrets([0x41; 32], [0x42; 64]);
    let identity = computer_tunnel.identity();
    let paired = phone_tunnel.identity().x25519();

    let connection = client::connect(relay.quic, relay.pin).await.unwrap();
    let mut listener = Listener::listen(connection, &computer_relay_key, b"")
        .await
        .unwrap();
    listener
        .set_dial_keys(vec![public(&phone_dial_key)])
        .await
        .unwrap();
    let route = listener.route();

    let computer = tokio::spawn(async move {
        let incoming = listener.accept().await.unwrap();
        let responder = Responder::session(&computer_tunnel, route.as_bytes()).unwrap();
        let mut tunnel = stream::accept(
            join(incoming.recv, incoming.send),
            &computer_tunnel,
            responder,
            |remote| *remote == paired,
        )
        .await
        .unwrap();
        let mut request = [0; 13];
        tunnel.io.read_exact(&mut request).await.unwrap();
        assert_eq!(&request, b"GET /api/v2/\n");
        tunnel.io.write_all(b"200 chats").await.unwrap();
        tunnel.io.shutdown().await.unwrap();
        (listener, tunnel)
    });

    let phone = client::connect(relay.quic, relay.pin).await.unwrap();
    let (send, recv) = client::dial(&phone, route, &phone_dial_key).await.unwrap();
    let (initiator, first) =
        Initiator::session(&phone_tunnel, &identity, route.as_bytes()).unwrap();
    let mut tunnel = stream::connect(join(recv, send), initiator, &first, identity.x25519())
        .await
        .unwrap();
    tunnel.io.write_all(b"GET /api/v2/\n").await.unwrap();
    let mut reply = Vec::new();
    tunnel.io.read_to_end(&mut reply).await.unwrap();
    assert_eq!(reply, b"200 chats");
    let _held = computer.await.unwrap();
    // Every byte the relay carried was ciphertext and padding: far more
    // than the 22 bytes of plaintext.
    let totals = relay.hub.metrics.totals();
    assert!(totals.bytes_from_dialers > 1_000, "{totals:?}");
}

#[tokio::test]
async fn a_dial_key_the_computer_did_not_name_is_refused_by_the_relay() {
    let relay = start_both(&options(Limits::default())).await;
    let computer_relay_key = signing_key(172);
    let connection = client::connect(relay.quic, relay.pin).await.unwrap();
    let mut listener = Listener::listen(connection, &computer_relay_key, b"")
        .await
        .unwrap();
    listener
        .set_dial_keys(vec![public(&signing_key(173))])
        .await
        .unwrap();
    let stranger = client::connect(relay.quic, relay.pin).await.unwrap();
    let refused = client::dial(&stranger, listener.route(), &signing_key(174)).await;
    assert!(matches!(
        refused,
        Err(client::Error::Refused(Status::Offline))
    ));
}
