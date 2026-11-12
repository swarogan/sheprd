//! Writing to a Gitea issue from its page, against a local server standing
//! in for the instance: the paths, the bodies, the one status a write wants,
//! and what is refused before any request is made.
use crate::{
    Provider, SourceKey,
    gitea::{
        self, Error as GiteaError,
        write::{Change, MAX_COMMENT, perform, perform_at},
    },
};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread::JoinHandle,
};

const TOKEN: &str = "0123456789abcdef";

fn source() -> SourceKey {
    SourceKey {
        provider: Provider::Gitea,
        host: "forge.example.com".into(),
        repository: "acme/app".into(),
    }
}

fn token() -> SecretString {
    SecretString::from(TOKEN)
}

/// Answers each connection with the next `(status, body)`, returning each
/// request whole: its head, a blank line, and its body.
fn serve(answers: Vec<(u16, String)>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        answers
            .into_iter()
            .map(|(status, body)| {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    let end = line.trim_end().is_empty();
                    request.push_str(&line);
                    if end {
                        break;
                    }
                }
                let mut payload = vec![0; length];
                reader.read_exact(&mut payload).unwrap();
                request.push_str(&String::from_utf8(payload).unwrap());
                (&stream)
                    .write_all(
                        format!(
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                request
            })
            .collect()
    });
    (base, server)
}

fn change(base: &str, change: &Change) -> Result<(), GiteaError> {
    perform_at(
        &gitea::agent(std::time::Duration::from_secs(5)),
        base,
        &token(),
        change,
    )
}

fn state(index: &str, closed: bool) -> Change {
    Change::SetState {
        source: source(),
        index: index.into(),
        closed,
    }
}

fn comment(text: &str) -> Change {
    Change::Comment {
        source: source(),
        index: "7".into(),
        text: text.into(),
    }
}

fn assign() -> Change {
    Change::Assign {
        source: source(),
        index: "7".into(),
    }
}

fn created() -> (u16, String) {
    (201, "{}".into())
}

fn body_of(request: &str) -> Value {
    let (_, body) = request.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap()
}

fn authorization(request: &str) -> Option<&str> {
    request
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
        .and_then(|line| line.split_once(": ").map(|(_, value)| value))
}

#[test]
fn closing_patches_the_state_and_reopening_sends_open() {
    for (closed, wanted) in [(true, "closed"), (false, "open")] {
        let (base, server) = serve(vec![created()]);
        change(&base, &state("7", closed)).unwrap();
        let requests = server.join().unwrap();
        assert!(
            requests[0].starts_with("PATCH /api/v1/repos/acme/app/issues/7 "),
            "{requests:?}"
        );
        assert_eq!(body_of(&requests[0]), json!({ "state": wanted }));
        assert_eq!(
            authorization(&requests[0]),
            Some(format!("token {TOKEN}").as_str())
        );
    }
}

#[test]
fn a_comment_is_posted_trimmed_as_plain_text() {
    let (base, server) = serve(vec![created()]);
    change(&base, &comment("  # Done\n\nsee `x`  ")).unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("POST /api/v1/repos/acme/app/issues/7/comments "),
        "{requests:?}"
    );
    assert_eq!(
        body_of(&requests[0]),
        json!({ "body": "# Done\n\nsee `x`" })
    );
    assert_eq!(
        authorization(&requests[0]),
        Some(format!("token {TOKEN}").as_str())
    );
}

#[test]
fn assigning_reads_the_token_owner_and_patches_the_assignees() {
    let (base, server) = serve(vec![(200, json!({ "login": "me" }).to_string()), created()]);
    change(&base, &assign()).unwrap();
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /api/v1/user "), "{requests:?}");
    assert!(
        requests[1].starts_with("PATCH /api/v1/repos/acme/app/issues/7 "),
        "{requests:?}"
    );
    assert_eq!(body_of(&requests[1]), json!({ "assignees": ["me"] }));
    for request in &requests {
        assert_eq!(
            authorization(request),
            Some(format!("token {TOKEN}").as_str())
        );
    }
}

#[test]
fn a_user_without_a_login_assigns_nothing() {
    let (base, server) = serve(vec![(200, "{}".into())]);
    assert!(matches!(
        change(&base, &assign()),
        Err(GiteaError::Authentication)
    ));
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn a_refusal_to_write_is_its_own_failure_with_its_own_text() {
    for status in [401, 403] {
        let (base, server) = serve(vec![(status, "{}".into())]);
        let error = change(&base, &state("7", true)).unwrap_err();
        assert!(matches!(error, GiteaError::Forbidden), "{error}");
        assert_eq!(
            error.to_string(),
            "Gitea refused the change; the token needs the write:issue scope, and your account needs permission on the repository"
        );
        server.join().unwrap();
    }
}

#[test]
fn only_201_is_taken_for_done() {
    for status in [200, 204] {
        let (base, server) = serve(vec![(status, "{}".into())]);
        assert!(matches!(
            change(&base, &comment("hi")),
            Err(GiteaError::Status(s)) if s == status
        ));
        server.join().unwrap();
    }
    let (base, server) = serve(vec![(404, "{}".into())]);
    assert!(matches!(
        change(&base, &comment("hi")),
        Err(GiteaError::NotFound { .. })
    ));
    server.join().unwrap();
}

/// Runs `change` against a listener that must see no connection.
fn refused(wanted: impl Fn(&GiteaError) -> bool, change_: &Change) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let error = change(&base, change_).unwrap_err();
    assert!(wanted(&error), "{change_:?}: {error}");
    let seen = listener.accept().map(|_| ()).unwrap_err();
    assert_eq!(seen.kind(), std::io::ErrorKind::WouldBlock, "{change_:?}");
}

#[test]
fn an_empty_or_oversized_comment_is_refused_before_any_request() {
    for text in ["", "   \n\t "] {
        refused(|e| matches!(e, GiteaError::Comment), &comment(text));
    }
    refused(
        |e| matches!(e, GiteaError::CommentLength { limit } if *limit == MAX_COMMENT),
        &comment(&"a".repeat(MAX_COMMENT + 1)),
    );
}

#[test]
fn a_bad_owner_repository_or_number_is_refused_before_any_request() {
    for repository in [
        "../app",
        "acme/..",
        "acme/a%2Fb",
        "/app",
        "acme/",
        "acme/a b",
        "",
    ] {
        let mut bad = source();
        bad.repository = repository.into();
        let change = Change::SetState {
            source: bad,
            index: "7".into(),
            closed: true,
        };
        refused(
            |e| matches!(e, GiteaError::Issue | GiteaError::Address(_)),
            &change,
        );
    }
    for index in ["7a", "", "-1", "7/comments", &"1".repeat(21)] {
        refused(|e| matches!(e, GiteaError::Issue), &state(index, true));
    }
}

#[test]
fn a_source_whose_instance_is_not_saved_asks_to_sign_in() {
    let error = perform(None, &comment("hi")).unwrap_err();
    assert!(
        matches!(&error, GiteaError::SignIn { repository } if repository == "acme/app"),
        "{error}"
    );
}
