use std::{net::TcpListener, thread};

use super::*;
use serde_json::json;

/// A server on a loopback port that runs `serve` with the accepted socket,
/// and a client connected to it.
fn pair(
    serve: impl FnOnce(&mut WebSocket<TcpStream>) + Send + 'static,
) -> (Connection<TcpStream>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        serve(&mut socket);
    });
    let url = Url::parse(&format!("ws://127.0.0.1:{port}/ws?orchestrationProtocol=2")).unwrap();
    (Connection::open(&url).unwrap(), server)
}

fn read(socket: &mut WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(text) = socket.read().unwrap() {
            return serde_json::from_str(text.as_str()).unwrap();
        }
    }
}

fn send(socket: &mut WebSocket<TcpStream>, frame: &Value) {
    socket
        .send(Message::Text(frame.to_string().into()))
        .unwrap();
}

#[test]
fn a_stream_chunk_is_acknowledged_before_the_next_is_sent() {
    let snapshot = crate::fixtures::values(crate::fixtures::SNAPSHOTS, "1");
    let (mut client, server) = pair(move |socket| {
        let request = read(socket);
        assert_eq!(request["_tag"], "Request");
        assert_eq!(request["tag"], "orchestration.subscribeShell");
        let id = request["id"].clone();
        send(
            socket,
            &json!({"_tag": "Chunk", "requestId": id, "values": snapshot}),
        );
        assert_eq!(read(socket), json!({"_tag": "Ack", "requestId": id}));
        send(
            socket,
            &json!({"_tag": "Chunk", "requestId": id,
            "values": [{"kind": "synchronized"}]}),
        );
        assert_eq!(read(socket), json!({"_tag": "Ack", "requestId": id}));
        assert_eq!(read(socket), json!({"_tag": "Interrupt", "requestId": id}));
    });
    let id = client
        .request("orchestration.subscribeShell", json!({}))
        .unwrap();
    let mut shell = crate::shell::Shell::default();
    for _ in 0..2 {
        let ServerFrame::Chunk { request_id, values } = client.next_frame().unwrap() else {
            panic!("a chunk");
        };
        assert_eq!(request_id, id);
        for value in values {
            shell.apply(value).unwrap();
        }
    }
    client.interrupt(&id).unwrap();
    server.join().unwrap();
    assert!(shell.threads.contains_key(crate::fixtures::THREAD));
}

#[test]
fn a_call_returns_its_value_or_the_servers_refusal() {
    let (mut client, server) = pair(|socket| {
        for _ in 0..2 {
            let request = read(socket);
            let id = request["id"].clone();
            // A stream's frames for another request come first.
            send(
                socket,
                &json!({"_tag": "Chunk", "requestId": "other", "values": []}),
            );
            assert_eq!(read(socket)["_tag"], "Ack");
            let exit = if request["payload"]["ok"] == true {
                json!({"_tag": "Success", "value": {"sequence": 50}})
            } else {
                json!({"_tag": "Failure", "cause": [{"_tag": "Die", "defect": "Missing key"}]})
            };
            send(
                socket,
                &json!({"_tag": "Exit", "requestId": id, "exit": exit}),
            );
        }
    });
    let value = client
        .call("orchestration.dispatchCommand", json!({"ok": true}))
        .unwrap();
    assert_eq!(value, json!({"sequence": 50}));
    let error = client
        .call("orchestration.dispatchCommand", json!({"ok": false}))
        .unwrap_err();
    assert!(matches!(
        error,
        Error::Call { ref cause, ref tag } if cause == "Missing key" && tag == "orchestration.dispatchCommand"
    ));
    server.join().unwrap();
}

#[test]
fn a_closed_socket_ends_the_wait() {
    let (mut client, server) = pair(|socket| {
        let _ = read(socket);
        socket.close(None).unwrap();
        let _ = socket.flush();
    });
    let error = client
        .call("orchestration.getThreadTitles", json!({}))
        .unwrap_err();
    assert!(matches!(error, Error::Closed), "{error:?}");
    server.join().unwrap();
}

#[test]
fn a_message_over_the_limit_is_refused() {
    let (mut client, server) = pair(|socket| {
        let _ = read(socket);
        let huge = "x".repeat(MAX_MESSAGE + 1);
        let _ = socket.send(Message::Text(huge.into()));
    });
    client
        .request("orchestration.subscribeShell", json!({}))
        .unwrap();
    let error = client.next_frame().unwrap_err();
    assert!(
        matches!(error, Error::TooLarge { limit: MAX_MESSAGE }),
        "{error:?}"
    );
    drop(client);
    server.join().unwrap();
}

#[test]
fn only_a_plain_local_socket_is_opened() {
    let url = Url::parse("wss://example.invalid/ws").unwrap();
    assert!(matches!(Connection::open(&url), Err(Error::Scheme(scheme)) if scheme == "wss"));
}

#[test]
fn a_quiet_server_is_idle_not_gone_and_answers_a_ping() {
    let (mut client, server) = pair(|socket| {
        let _ = read(socket);
        // Nothing for longer than the client's read timeout, then the ping.
        assert_eq!(read(socket), json!({"_tag": "Ping"}));
        send(socket, &json!({"_tag": "Pong"}));
    });
    client
        .socket
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    client
        .request("orchestration.subscribeShell", json!({}))
        .unwrap();
    assert!(matches!(client.next_frame(), Err(Error::Idle)));
    client.ping().unwrap();
    assert_eq!(client.next_frame().unwrap(), ServerFrame::Pong);
    server.join().unwrap();
}

#[test]
fn open_tries_every_resolved_address() {
    let mut tried = Vec::new();
    let stream = connect_any(["::1", "127.0.0.1"], |address| {
        tried.push(*address);
        if *address == "::1" {
            Err(std::io::ErrorKind::ConnectionRefused.into())
        } else {
            Ok(*address)
        }
    })
    .unwrap();
    assert_eq!(stream, "127.0.0.1");
    assert_eq!(tried, ["::1", "127.0.0.1"]);
}

#[test]
fn open_keeps_the_last_failure_or_reports_no_server() {
    let error = connect_any(["::1", "127.0.0.1"], |_| -> std::io::Result<()> {
        Err(std::io::ErrorKind::ConnectionRefused.into())
    })
    .unwrap_err();
    assert!(
        matches!(&error, Error::Connect(source) if source.kind() == std::io::ErrorKind::ConnectionRefused)
    );
    let error = connect_any(std::iter::empty::<&str>(), |_| -> std::io::Result<()> {
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::NoServer));
}
