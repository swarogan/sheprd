//! A relay and a computer behind it that echoes what crosses its tunnel,
//! for checking another client (the iOS app's Swift dialer) against the
//! real thing: `cargo run -p sheprd-relay --example echo_computer
//! --features client,server`. It prints one JSON line (the relay's
//! endpoint, the computer's route and tunnel identity) and serves until
//! killed. The phone's keys are fixed: dial key scalar `[0x61; 32]`,
//! tunnel secret `[0x62; 32]`.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::SigningKey;
use sheprd_relay::{
    Options,
    client::{self, Listener, public},
};
use sheprd_tunnel::{Keys, Responder, stream};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, join},
    net::TcpListener,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tcp = TcpListener::bind("127.0.0.1:0").await?;
    let http = tcp.local_addr()?;
    let (quic_endpoint, info) = sheprd_relay::bind_quic("127.0.0.1:0".parse()?, None, None)?;
    let quic = quic_endpoint.local_addr()?;
    let pin: [u8; 32] = URL_SAFE_NO_PAD
        .decode(&info.pin)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("pin"))?;
    let options = Options {
        metrics: None,
        public_host: Some(http.to_string()),
        ..Options::default()
    };
    let (router, hub) = sheprd_relay::app(&options, Some(info))?;
    tokio::spawn(sheprd_relay::quic::run(quic_endpoint, hub));
    tokio::spawn(async move {
        let _ = axum::serve(
            tcp,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    let route_key = SigningKey::from_slice(&[0x71; 32])?;
    let computer = Keys::from_secrets([0x72; 32], [0x73; 64]);
    let phone_dial = SigningKey::from_slice(&[0x61; 32])?;
    let phone_tunnel = Keys::from_secrets([0x62; 32], [0x00; 64])
        .identity()
        .x25519();
    let connection = client::connect(quic, pin).await?;
    let mut listener = Listener::listen(connection, &route_key, b"").await?;
    listener.set_dial_keys(vec![public(&phone_dial)]).await?;
    let route = listener.route();
    println!(
        "{}",
        serde_json::json!({
            "endpoint": format!("http://{http}"),
            "route": route.to_string(),
            "identity": URL_SAFE_NO_PAD.encode(computer.identity().to_bytes()),
        })
    );
    let computer = std::sync::Arc::new(computer);
    loop {
        let incoming = listener.accept().await?;
        let computer = std::sync::Arc::clone(&computer);
        tokio::spawn(async move {
            let Ok(responder) = Responder::session(&computer, route.as_bytes()) else {
                return;
            };
            let io = join(incoming.recv, incoming.send);
            let Ok(mut tunnel) =
                stream::accept(io, &computer, responder, |remote| *remote == phone_tunnel).await
            else {
                return;
            };
            let mut buffer = vec![0; 65_536];
            while let Ok(read) = tunnel.io.read(&mut buffer).await {
                if read == 0 || tunnel.io.write_all(&buffer[..read]).await.is_err() {
                    break;
                }
            }
        });
    }
}
