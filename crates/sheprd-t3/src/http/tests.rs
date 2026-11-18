use std::{
    io::{BufRead as _, BufReader, Write as _},
    net::TcpListener,
    thread,
};

use super::*;

/// What the test server saw of one request.
struct Seen {
    body: String,
    head: String,
}

/// A server that answers one request with `status` and `body`, and hands
/// back what it was sent.
fn answer(status: u16, body: String) -> (Server, thread::JoinHandle<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            head.push_str(&line);
        }
        let mut sent = vec![0; length];
        std::io::Read::read_exact(&mut reader, &mut sent).unwrap();
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        Seen {
            body: String::from_utf8(sent).unwrap(),
            head,
        }
    });
    (Server::new(origin), handle)
}

fn descriptor(protocol: u32) -> String {
    serde_json::json!({"environmentId": "env", "label": "box", "serverVersion": "0.1.7",
        "orchestrationProtocolVersion": protocol, "capabilities": {}})
    .to_string()
}

#[test]
fn a_server_describes_itself_and_another_protocol_is_refused() {
    let (server, seen) = answer(200, descriptor(2));
    let described = server.describe().unwrap();
    assert_eq!(described.environment_id, "env");
    assert!(
        seen.join()
            .unwrap()
            .head
            .starts_with("GET /.well-known/t3/environment ")
    );
    let (server, _) = answer(200, descriptor(3));
    assert!(matches!(
        server.describe(),
        Err(Error::Protocol {
            expected: 2,
            found: 3
        })
    ));
}

#[test]
fn signing_in_exchanges_the_code_for_a_bearer_token_with_our_scopes() {
    let body = serde_json::json!({"access_token": "tok", "issued_token_type":
        "urn:ietf:params:oauth:token-type:access_token", "token_type": "Bearer",
        "expires_in": 2_591_999, "scope": SCOPES})
    .to_string();
    let (server, seen) = answer(200, body);
    let access = server
        .sign_in(&SecretString::from("95LC5R9AFWCN".to_owned()), "Sheprd")
        .unwrap();
    assert_eq!(access.token.expose_secret(), "tok");
    assert_eq!(access.expires_in, 2_591_999);
    let seen = seen.join().unwrap();
    assert!(seen.head.starts_with("POST /oauth/token "));
    let fields: Vec<(String, String)> = url::form_urlencoded::parse(seen.body.as_bytes())
        .into_owned()
        .collect();
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(field("subject_token"), Some("95LC5R9AFWCN"));
    assert_eq!(
        field("subject_token_type"),
        Some("urn:t3:params:oauth:token-type:environment-bootstrap")
    );
    assert_eq!(field("scope"), Some(SCOPES));
}

#[test]
fn a_bound_token_a_refusal_or_an_oversized_answer_is_an_error() {
    let dpop = serde_json::json!({"access_token": "tok", "token_type": "DPoP"}).to_string();
    let (server, _) = answer(200, dpop);
    let code = SecretString::from("code".to_owned());
    assert!(matches!(server.sign_in(&code, "S"), Err(Error::TokenType(kind)) if kind == "DPoP"));
    let (server, _) = answer(401, "{}".to_owned());
    assert!(matches!(
        server.sign_in(&code, "S"),
        Err(Error::Status {
            path: "/oauth/token",
            status: 401
        })
    ));
    let (server, _) = answer(200, format!("\"{}\"", "x".repeat(LIMIT as usize + 1)));
    assert!(matches!(server.describe(), Err(Error::TooLarge { .. })));
}

#[test]
fn a_ticket_is_bought_with_the_token_and_opens_the_socket() {
    let (server, seen) = answer(200, r#"{"ticket":"tick et","expiresAt":"x"}"#.to_owned());
    let ticket = server
        .ticket(&SecretString::from("tok".to_owned()))
        .unwrap();
    assert!(
        seen.join()
            .unwrap()
            .head
            .contains("authorization: Bearer tok")
    );
    let url = server.socket(&ticket).unwrap();
    assert_eq!(url.scheme(), "ws");
    assert_eq!(url.path(), "/ws");
    assert_eq!(
        url.query(),
        Some("orchestrationProtocol=2&wsTicket=tick+et")
    );
    let secure = Server::new(Url::parse("https://box.example").unwrap());
    assert_eq!(secure.socket(&ticket).unwrap().scheme(), "wss");
}
