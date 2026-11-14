//! The vectors the iOS app's relay dialer is checked against: the name a
//! WebSocket proof covers for each endpoint form, and the exact bytes a
//! dialer signs. `HERDR_UPDATE_FIXTURES=1` rewrites them.

use crate::{Endpoint, RouteId, socket};
use serde_json::{Value, json};
use std::path::PathBuf;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn check(name: &str, value: &Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ios/SheprdRemoteCore/Tests/SheprdRemoteCoreTests/Fixtures/relay")
        .join(format!("{name}.json"));
    let text = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var_os("HERDR_UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let expected: Value = serde_json::from_str(&expected).unwrap();
    assert_eq!(
        value,
        &expected,
        "{} is out of date; run with HERDR_UPDATE_FIXTURES=1",
        path.display()
    );
}

#[test]
fn dial_vectors() {
    let endpoints: Vec<Value> = [
        "relay.example.org",
        "https://Relay.Example.org:443/",
        "https://relay.example.org:8443/sheprd/",
        "wss://161-35-226-62.sslip.io",
        "http://127.0.0.1:8080",
    ]
    .iter()
    .map(|text| {
        let endpoint = Endpoint::parse(text).unwrap();
        json!({
            "text": text,
            "proof_name": endpoint.proof_name(),
            "dial": endpoint.dial(&RouteId::from_bytes([7; 32])).to_string(),
        })
    })
    .collect();
    let nonce = [0x4e; socket::NONCE_LEN];
    let route = RouteId::from_bytes([0x52; 32]);
    let name = "relay.example.org/sheprd";
    check(
        "dial",
        &json!({
            "endpoints": endpoints,
            "proof_name": name,
            "nonce": hex(&nonce),
            "route": hex(route.as_bytes()),
            "route_text": route.to_string(),
            "signed": hex(&socket::dial_signed(name, &nonce, &route)),
        }),
    );
}

#[test]
fn pairing_dial_vectors() {
    let secret = [0x5e; 32];
    let key = crate::route::pairing_dial_key(&secret);
    check(
        "pairing",
        &json!({
            "secret": hex(&secret),
            "scalar": hex(&key.to_bytes()),
            "public": hex(key.verifying_key().to_sec1_point(true).as_bytes()),
        }),
    );
}
