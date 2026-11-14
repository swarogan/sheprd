//! The daemon on a real relay on loopback: a paired phone that sent its
//! relay keys reaches the API end to end; anyone else does not.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::tls::{Authority, Names, Resolver};
use crate::web::tests::{post, send, signed_in, state};
use axum::http::{StatusCode, header};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::SigningKey;
use rustls::pki_types::ServerName;
use sheprd_relay::{
    Options as RelayOptions, RouteId,
    client::{self, public},
};
use sheprd_tunnel::{Identity, Initiator, Keys as TunnelKeys, stream};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, join},
    net::TcpListener,
};

mod addresses;

/// A relay serving QUIC and HTTP on loopback ports: its HTTP address, and
/// its QUIC address and pin.
async fn relay() -> (SocketAddr, SocketAddr, sheprd_relay::pin::Pin) {
    let (endpoint, info) =
        sheprd_relay::bind_quic("127.0.0.1:0".parse().unwrap(), None, None).unwrap();
    let quic = endpoint.local_addr().unwrap();
    let pin = URL_SAFE_NO_PAD
        .decode(&info.pin)
        .unwrap()
        .try_into()
        .unwrap();
    let options = RelayOptions {
        metrics: None,
        ..RelayOptions::default()
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

/// What a relay session serves on `shared`, behind the certificate the
/// daemon presents for `127.0.0.1`; and a client config that trusts only
/// this computer's authority, as the phone does.
fn served(dir: &Path, shared: &crate::web::Shared) -> (Served, rustls::ClientConfig) {
    let authority = Authority::load_or_create(dir).unwrap();
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let resolver = Arc::new(
        Resolver::issue(
            &authority,
            Names::new([loopback], Some("studio")),
            SystemTime::now(),
        )
        .unwrap(),
    );
    let mut tls = crate::tls::server_config(resolver.clone()).unwrap();
    tls.cert_resolver = resolver.at(loopback);
    let mut roots = rustls::RootCertStore::empty();
    roots.add(authority.ca_der().to_vec().into()).unwrap();
    let client = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    let served = Served {
        router: crate::web::router(shared.clone()),
        state: shared.clone(),
        tls: Arc::new(tls),
    };
    (served, client)
}

async fn listening(relays: &Relays) {
    for _ in 0..100 {
        if relays
            .status()
            .values()
            .any(|state| *state == Status::Listening)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never listened: {:?}", relays.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paired_phone_reaches_the_api_through_a_relay_end_to_end() {
    let (http, quic, pin) = relay().await;
    let dir = tempfile::tempdir().unwrap();
    let mut shared = state(dir.path());
    let endpoint = Endpoint::parse(&format!("http://{http}")).unwrap();
    let relays = Arc::new(Relays::load(dir.path(), vec![endpoint]).unwrap());
    Arc::get_mut(&mut shared).unwrap().relay = Some(Arc::clone(&relays));
    let (_device, token) = signed_in(&shared, 9);

    // The phone, signed in over the network, learns the relay and sends
    // its keys.
    let phone_tunnel = TunnelKeys::from_secrets([0x51; 32], [0x52; 64]);
    let phone_dial = SigningKey::from_slice(&[0x53; 32]).unwrap();
    let mut request = crate::web::tests::get("/api/relay");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    let (status, _, body) = send(&shared, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let info: serde_json::Value = serde_json::from_str(&body).unwrap();
    let identity = Identity::from_bytes(
        &URL_SAFE_NO_PAD
            .decode(info["identity"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let route = RouteId::parse(info["route"].as_str().unwrap()).unwrap();
    let mut enroll = post(
        "/api/relay",
        serde_json::json!({
            "dial": URL_SAFE_NO_PAD.encode(public(&phone_dial)),
            "tunnel": URL_SAFE_NO_PAD.encode(phone_tunnel.identity().x25519()),
        }),
    );
    enroll.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    assert_eq!(send(&shared, enroll).await.0, StatusCode::OK);

    let (_stop, stopped) = watch::channel(false);
    let (served, client) = served(dir.path(), &shared);
    relays.start(served, stopped);
    listening(&relays).await;

    // Away from home: through the relay, inside the tunnel, the same API
    // over the same TLS the phone pins.
    let connection = client::connect(quic, pin).await.unwrap();
    let (send_half, recv_half) = client::dial(&connection, route, &phone_dial).await.unwrap();
    let (initiator, first) =
        Initiator::session(&phone_tunnel, &identity, route.as_bytes()).unwrap();
    let tunnel = stream::connect(
        join(recv_half, send_half),
        initiator,
        &first,
        identity.x25519(),
    )
    .await
    .unwrap();
    let mut https = tokio_rustls::TlsConnector::from(Arc::new(client))
        .connect(ServerName::IpAddress(Ipv4Addr::LOCALHOST.into()), tunnel.io)
        .await
        .unwrap();
    let request = format!(
        "GET /api/relay HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    https.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    // A peer closing TLS without close_notify still ends the read.
    let _ = https.read_to_end(&mut response).await;
    let response = String::from_utf8_lossy(&response);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(
        response.contains(info["route"].as_str().unwrap()),
        "{response}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dial_key_no_device_sent_is_refused_at_the_relay() {
    let (http, quic, pin) = relay().await;
    let dir = tempfile::tempdir().unwrap();
    let shared = state(dir.path());
    let endpoint = Endpoint::parse(&format!("http://{http}")).unwrap();
    let relays = Arc::new(Relays::load(dir.path(), vec![endpoint]).unwrap());
    let (_stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &shared).0, stopped);
    listening(&relays).await;
    let route = relays.keys.route();
    let connection = client::connect(quic, pin).await.unwrap();
    let stranger = SigningKey::from_slice(&[0x77; 32]).unwrap();
    assert!(matches!(
        client::dial(&connection, route, &stranger).await,
        Err(client::Error::Refused(sheprd_relay::wire::Status::Offline))
    ));
}

#[test]
fn keys_are_kept_across_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let first = Relays::load(dir.path(), Vec::new()).unwrap();
    let again = Relays::load(dir.path(), Vec::new()).unwrap();
    assert_eq!(first.info().route, again.info().route);
    assert_eq!(first.info().identity, again.info().identity);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_nearby_phone_off_the_network_claims_through_a_pairing_dial() {
    let (http, quic, pin) = relay().await;
    let dir = tempfile::tempdir().unwrap();
    let mut shared = state(dir.path());
    let endpoint = Endpoint::parse(&format!("http://{http}")).unwrap();
    let relays = Arc::new(Relays::load(dir.path(), vec![endpoint]).unwrap());
    Arc::get_mut(&mut shared).unwrap().relay = Some(Arc::clone(&relays));
    let (_stop, stopped) = watch::channel(false);
    let (served, client) = served(dir.path(), &shared);
    relays.start(served, stopped);
    listening(&relays).await;

    // The token sealed to the phone over Bluetooth is the pairing secret.
    let secret = [0x5e; 32];
    relays.offer_pairing(secret);
    let info = relays.info();
    let identity = Identity::from_bytes(&URL_SAFE_NO_PAD.decode(&info.identity).unwrap()).unwrap();
    let route = RouteId::parse(&info.route).unwrap();
    let dial = sheprd_relay::route::pairing_dial_key(&secret);
    let phone_tunnel = TunnelKeys::from_secrets([0x54; 32], [0x55; 64]);
    let connection = client::connect(quic, pin).await.unwrap();
    // The listener names the key moments after it is offered.
    let mut dialed = None;
    for _ in 0..50 {
        if let Ok(halves) = client::dial(&connection, route, &dial).await {
            dialed = Some(halves);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (send_half, recv_half) = dialed.expect("the pairing dial was never let through");
    let (initiator, first) = Initiator::pairing(
        &phone_tunnel,
        &identity.x25519(),
        &identity.fingerprint(),
        route.as_bytes(),
        &secret,
    )
    .unwrap();
    let tunnel = stream::connect(
        join(recv_half, send_half),
        initiator,
        &first,
        identity.x25519(),
    )
    .await
    .unwrap();
    let mut https = tokio_rustls::TlsConnector::from(Arc::new(client))
        .connect(ServerName::IpAddress(Ipv4Addr::LOCALHOST.into()), tunnel.io)
        .await
        .unwrap();
    https
        .write_all(b"GET /api/ping HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    let _ = https.read_to_end(&mut response).await;
    let response = String::from_utf8_lossy(&response);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

    // One dial: the key is spent.
    let again = client::dial(&connection, route, &dial).await;
    assert!(matches!(
        again,
        Err(client::Error::Refused(sheprd_relay::wire::Status::Offline))
    ));
}

#[test]
fn a_nearby_offer_names_the_relays_and_hands_its_token_to_them_once() {
    let dir = tempfile::tempdir().unwrap();
    let relays = Relays::load(dir.path(), Vec::new()).unwrap();
    let mut auth = crate::auth::Auth::load(dir.path(), crate::auth::Limits::default()).unwrap();
    assert!(auth.take_relay_pairing().is_none());
    auth.set_relay_offer(relays.info());
    let offer = serde_json::to_value(crate::auth::nearby::Offer {
        token: "t".into(),
        origins: &[],
        relay: Some(&relays.info()),
    })
    .unwrap();
    assert_eq!(offer["relay"]["route"], relays.info().route);
    let without = serde_json::to_value(crate::auth::nearby::Offer {
        token: "t".into(),
        origins: &[],
        relay: None,
    })
    .unwrap();
    assert!(without.get("relay").is_none());
}
