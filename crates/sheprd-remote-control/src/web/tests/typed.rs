//! Where a phone's prompt says it was typed.

use super::*;
use axum::{Extension, routing::get};
use sheprd_chat_protocol::{Device, Origin, Via};

const SAFARI: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15";
const APP: &str = "Sheprd/42 CFNetwork/3826.400.120 Darwin/25.0.0";

fn ip(text: &str) -> Option<IpAddr> {
    Some(text.parse().unwrap())
}

#[test]
fn the_app_and_the_web_app_are_told_apart() {
    assert_eq!(crate::web::typed::device(Some(SAFARI)), Device::PhoneWeb);
    assert_eq!(crate::web::typed::device(Some(APP)), Device::PhoneApp);
    assert_eq!(crate::web::typed::device(None), Device::PhoneApp);
}

#[test]
fn the_connection_is_named_by_the_relay_the_address_or_the_name_dialed() {
    use crate::web::typed::via;
    assert_eq!(via(true, ip("100.101.102.103"), None), Via::Relay);
    assert_eq!(via(false, ip("100.101.102.103"), None), Via::Tailscale);
    assert_eq!(via(false, ip("fd7a:115c:a1e0::1"), None), Via::Tailscale);
    assert_eq!(
        via(false, ip("192.168.1.50"), Some("mac.tail1234.ts.net:7443")),
        Via::Tailscale
    );
    assert_eq!(
        via(false, ip("192.168.1.50"), Some("192.168.1.20:7443")),
        Via::Lan
    );
    assert_eq!(via(false, ip("::ffff:10.0.0.4"), None), Via::Lan);
    assert_eq!(via(false, ip("fd12::4"), None), Via::Lan);
    assert_eq!(via(false, ip("127.0.0.1"), None), Via::Local);
    // Outside the shared space 100.64.0.0/10.
    assert_eq!(via(false, ip("100.128.0.1"), None), Via::Other);
    assert_eq!(via(false, ip("203.0.113.9"), None), Via::Other);
    assert_eq!(via(false, None, None), Via::Other);
}

async fn origin_of(request: HttpRequest<Body>, relayed: bool) -> Origin {
    let mut router = Router::new().route(
        "/",
        get(|TypedFrom(origin): TypedFrom| async move { axum::Json(origin) }),
    );
    if relayed {
        router = router.layer(Extension(Relayed));
    }
    let response = router.oneshot(request).await.unwrap();
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn request(user_agent: &str, peer: [u8; 4]) -> HttpRequest<Body> {
    let mut request = HttpRequest::get("/")
        .header("user-agent", user_agent)
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(Peer(SocketAddr::from((peer, 50_000)))));
    request
}

#[tokio::test]
async fn a_request_carries_its_origin_to_the_handler() {
    assert_eq!(
        origin_of(request(SAFARI, [192, 168, 1, 50]), false).await,
        Origin::new(Device::PhoneWeb, Via::Lan)
    );
    // The relay gives each device a private address; its marker wins.
    assert_eq!(
        origin_of(request(APP, [192, 168, 1, 50]), true).await,
        Origin::new(Device::PhoneApp, Via::Relay)
    );
}
