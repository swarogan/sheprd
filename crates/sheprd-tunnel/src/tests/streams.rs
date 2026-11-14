//! The tunnel over a Tokio stream, as the daemon runs it.

use super::*;
use crate::{
    Error, Initiator, Responder,
    stream::{accept, connect},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

#[tokio::test]
async fn bytes_cross_a_tunnel_both_ways_and_close_cleanly() {
    let (phone, computer) = (keys(1), keys(2));
    let (dialer_side, listener_side) = duplex(4_096);
    let listener = tokio::spawn(async move {
        let computer = keys(2);
        let paired = keys(1).identity().x25519();
        let responder = Responder::session(&computer, &ROUTE).unwrap();
        accept(listener_side, &computer, responder, |remote| {
            *remote == paired
        })
        .await
    });
    let identity = computer.identity();
    let (initiator, first) = Initiator::session(&phone, &identity, &ROUTE).unwrap();
    let mut dialing = connect(dialer_side, initiator, &first, identity.x25519())
        .await
        .unwrap();
    let mut listening = listener.await.unwrap().unwrap();
    assert_eq!(dialing.hash, listening.hash);
    assert_eq!(listening.remote, phone.identity().x25519());

    // More than one record's worth, to cross record boundaries.
    let big = vec![0x42; 100_000];
    let writer = {
        let big = big.clone();
        tokio::spawn(async move {
            dialing.io.write_all(&big).await.unwrap();
            dialing.io.shutdown().await.unwrap();
            dialing
        })
    };
    let mut received = Vec::new();
    listening.io.read_to_end(&mut received).await.unwrap();
    assert_eq!(received, big);
    let dialing = writer.await.unwrap();
    drop(dialing.io);
    assert!(listening.task.await.unwrap().is_ok());
}

#[tokio::test]
async fn an_unpaired_dialer_is_refused_and_hears_nothing() {
    let (stranger, computer) = (keys(9), keys(2));
    let (dialer_side, listener_side) = duplex(4_096);
    let listener = tokio::spawn(async move {
        let computer = keys(2);
        let responder = Responder::session(&computer, &ROUTE).unwrap();
        accept(listener_side, &computer, responder, |_| false).await
    });
    let identity = computer.identity();
    let (initiator, first) = Initiator::session(&stranger, &identity, &ROUTE).unwrap();
    let listener_key = identity.x25519();
    let dialing =
        tokio::spawn(async move { connect(dialer_side, initiator, &first, listener_key).await });
    assert!(matches!(listener.await.unwrap(), Err(Error::Refused)));
    // The listener dropped its side without answering.
    assert!(matches!(dialing.await.unwrap(), Err(Error::Closed)));
}

/// A paired phone and computer with a tunnel between them, over a stream
/// that holds `capacity` bytes each way.
async fn tunnels(capacity: usize) -> (crate::stream::Tunnel, crate::stream::Tunnel) {
    let (phone, computer) = (keys(1), keys(2));
    let (dialer_side, listener_side) = duplex(capacity);
    let listener = tokio::spawn(async move {
        let computer = keys(2);
        let responder = Responder::session(&computer, &ROUTE).unwrap();
        accept(listener_side, &computer, responder, |_| true).await
    });
    let identity = computer.identity();
    let (initiator, first) = Initiator::session(&phone, &identity, &ROUTE).unwrap();
    let dialing = connect(dialer_side, initiator, &first, identity.x25519())
        .await
        .unwrap();
    (dialing, listener.await.unwrap().unwrap())
}

#[tokio::test]
async fn both_sides_sending_at_once_do_not_wait_on_each_other() {
    // A stream far smaller than what each side sends: each pump's write
    // waits for the other pump to read, which it must keep doing.
    let (dialing, listening) = tunnels(1_024).await;
    let exchange = |tunnel: crate::stream::Tunnel, byte: u8, expected: u8| async move {
        let (mut read, mut write) = tokio::io::split(tunnel.io);
        let sent = vec![byte; 1_000_000];
        let writer = tokio::spawn(async move {
            write.write_all(&sent).await.unwrap();
            write
        });
        let mut received = vec![0; 1_000_000];
        read.read_exact(&mut received).await.unwrap();
        assert!(received.iter().all(|byte| *byte == expected));
        writer.await.unwrap();
    };
    let both = async {
        tokio::join!(
            exchange(dialing, 0x11, 0x22),
            exchange(listening, 0x22, 0x11)
        );
    };
    tokio::time::timeout(std::time::Duration::from_secs(30), both)
        .await
        .expect("two-way traffic froze");
}
