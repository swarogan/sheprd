//! `GET /api/v2/github/token` and what `home` says about GitHub.

use super::*;
use crate::{
    control::{Request, Response},
    hub::Workspace,
    native::github::{Lending, checkouts::Checkout, hosts::GhHost, remote::Repository},
};
use secrecy::SecretString;
use std::time::Duration;

const SHEPRD_TOKEN: &str = "ghu_fromsheprd";
const GH_TOKEN: &str = "gho_fromcli";

fn gh_signed_in(_: &str) -> sheprd_github::Result<SecretString> {
    Ok(GH_TOKEN.into())
}

fn gh_signed_out(_: &str) -> sheprd_github::Result<SecretString> {
    Err(sheprd_github::cli::Error::SignedOut.into())
}

fn lending_state(dir: &std::path::Path, lending: Lending) -> Shared {
    state_lending(
        dir,
        Hub::offline(Layout {
            connected: true,
            boot: "b".into(),
            working: 0,
            workspaces: vec![Workspace {
                id: "w1".into(),
                label: "App".into(),
                cwd: "/code/app".into(),
                branch: Some("topic".into()),
                worktree: None,
                session: None,
                chats: Vec::new(),
                terminals: Vec::new(),
            }],
        }),
        lending,
    )
}

fn authed(token: &str, path: &str) -> HttpRequest<Body> {
    HttpRequest::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn control(state: &Shared, request: Request) -> Response {
    let (stop, _) = tokio::sync::watch::channel(false);
    crate::server::answer_for_tests(state, &stop, request)
}

/// Sheprd is open with `login` as its main account, as its polls say.
fn sheprd_open(state: &Shared, login: &str) {
    control(state, Request::Status);
    state
        .lending
        .asks
        .take(Some(login.to_owned()), Duration::ZERO);
}

/// Stands in for Sheprd: takes one ask and answers it with `answer`.
fn sheprd_answers(state: &Shared, answer: crate::control::github::LeaseAnswer) {
    let state = state.clone();
    std::thread::spawn(move || {
        let Response::GitHubLeaseAsks { asks } = control(
            &state,
            Request::GitHubLeaseAsks {
                login: Some("octocat".into()),
            },
        ) else {
            panic!("the daemon did not hand over the ask");
        };
        let ask = asks.first().expect("an ask");
        assert_eq!(ask.host, "github.com");
        assert_eq!(
            control(&state, Request::GitHubLease { id: ask.id, answer }),
            Response::Done
        );
    });
}

fn lease() -> crate::control::github::LeaseAnswer {
    crate::control::github::LeaseAnswer::Lease(crate::control::github::Lease {
        login: "octocat".into(),
        token: crate::control::github::Token::new(SHEPRD_TOKEN.into()),
        expires_at: Some(1_800_000_000),
    })
}

async fn lent(state: &Shared, token: &str, host: &str) -> (StatusCode, serde_json::Value) {
    let path = format!("/api/v2/github/token?host={host}");
    let (status, _, body) = send(state, authed(token, &path)).await;
    (status, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn the_token_needs_a_session() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
    let (status, ..) = send(&state, get("/api/v2/github/token?host=github.com")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sheprd_lends_its_main_account_for_github_com() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
    let (_, token) = signed_in(&state, 1);
    sheprd_open(&state, "octocat");
    sheprd_answers(&state, lease());
    let (status, body) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        serde_json::json!({
            "host": "github.com",
            "login": "octocat",
            "token": SHEPRD_TOKEN,
            "expires_at": 1_800_000_000u64,
            "source": "sheprd",
        })
    );
}

#[tokio::test]
async fn gh_lends_when_sheprd_is_signed_out_or_closed() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
    state.lending.set_gh_hosts(vec![
        GhHost {
            host: "github.com".into(),
            login: "cli-me".into(),
        },
        GhHost {
            host: "ghe.example.com".into(),
            login: "corp-me".into(),
        },
    ]);
    let (_, token) = signed_in(&state, 1);
    // Closed: gh answers at once.
    let (status, body) = lent(&state, &token, "GHE.example.com").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        serde_json::json!({
            "host": "ghe.example.com",
            "login": "corp-me",
            "token": GH_TOKEN,
            "source": "gh",
        })
    );
    // Open but signed out of GitHub.
    sheprd_open(&state, "octocat");
    sheprd_answers(&state, crate::control::github::LeaseAnswer::SignedOut);
    let (status, body) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["source"].as_str(), body["login"].as_str()),
        (Some("gh"), Some("cli-me"))
    );
}

#[tokio::test]
async fn a_device_whose_switch_is_off_gets_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
    state.lending.set_gh_hosts(vec![GhHost {
        host: "github.com".into(),
        login: "cli-me".into(),
    }]);
    let (device, token) = signed_in(&state, 1);
    let share = |share| Request::ShareGitHub {
        device: device.clone(),
        share,
    };
    assert_eq!(control(&state, share(false)), Response::Done);
    let (status, body) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.to_string().contains(GH_TOKEN));
    let (_, _, home) = send(&state, authed(&token, "/api/v2/home")).await;
    let home: serde_json::Value = serde_json::from_str(&home).unwrap();
    assert!(home.get("github_hosts").is_none(), "{home}");

    assert_eq!(control(&state, share(true)), Response::Done);
    let (status, _) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::OK);
    // A device the daemon does not know is refused, and nothing changes.
    let unknown = control(
        &state,
        Request::ShareGitHub {
            device: DeviceId("gone".into()),
            share: true,
        },
    );
    assert!(matches!(
        unknown,
        Response::Error {
            code: crate::control::ErrorCode::UnknownDevice,
            ..
        }
    ));
}

#[tokio::test]
async fn a_revoked_device_gets_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
    let (device, token) = signed_in(&state, 1);
    assert_eq!(control(&state, Request::Revoke { device }), Response::Done);
    let (status, _) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn no_account_for_the_host_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_out));
    let (_, token) = signed_in(&state, 1);
    for host in [
        "github.com",
        "acme.ghe.com",
        "ghe.example.com",
        "gitlab.com",
    ] {
        let (status, _) = lent(&state, &token, host).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{host}");
    }
    let (status, _) = lent(&state, &token, "https://github.com").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_device_asks_a_few_times_a_minute_at_most() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_out));
    let (_, token) = signed_in(&state, 1);
    let (_, other) = signed_in(&state, 2);
    for _ in 0..5 {
        let (status, _) = lent(&state, &token, "github.com").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, _) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    // Each device has its own allowance.
    let (status, _) = lent(&state, &other, "github.com").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sheprd_silent_without_gh_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let lending = Lending::new(Vec::new(), gh_signed_out).waiting(Duration::from_millis(100));
    let state = lending_state(dir.path(), lending);
    let (_, token) = signed_in(&state, 1);
    sheprd_open(&state, "octocat");
    let (status, _) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
    // The phone that gave up leaves nothing queued.
    assert_eq!(state.lending.asks.queued(), 0);
}

#[tokio::test]
async fn sheprd_silent_with_gh_lends_gh() {
    let dir = tempfile::tempdir().unwrap();
    let lending = Lending::new(Vec::new(), gh_signed_in).waiting(Duration::from_millis(100));
    let state = lending_state(dir.path(), lending);
    let (_, token) = signed_in(&state, 1);
    sheprd_open(&state, "octocat");
    let (status, body) = lent(&state, &token, "github.com").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["source"], "gh");
}

#[tokio::test]
async fn sheprds_asks_show_in_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let lending = Lending::new(Vec::new(), gh_signed_out).waiting(Duration::from_secs(5));
    let state = lending_state(dir.path(), lending);
    let (_, token) = signed_in(&state, 1);
    sheprd_open(&state, "octocat");
    let asking = tokio::spawn({
        let state = state.clone();
        async move { lent(&state, &token, "github.com").await }
    });
    let mut waiting = 0;
    for _ in 0..200 {
        let Response::Status(status) = control(&state, Request::Status) else {
            panic!("no status");
        };
        waiting = status.github_asks;
        if waiting > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(waiting, 1);
    sheprd_answers(&state, lease());
    let (status, _) = asking.await.unwrap();
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn home_names_each_workspaces_repository_and_the_accounts_lent() {
    let dir = tempfile::tempdir().unwrap();
    let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
    let (_, token) = signed_in(&state, 1);
    let home = |state: Shared, token: String| async move {
        let (_, _, body) = send(&state, authed(&token, "/api/v2/home")).await;
        serde_json::from_str::<serde_json::Value>(&body).unwrap()
    };
    // Nothing read yet: neither field.
    let empty = home(state.clone(), token.clone()).await;
    assert!(empty.get("github_hosts").is_none());
    assert!(empty["workspaces"][0].get("github").is_none());

    let repository = |host: &str| Repository {
        host: host.into(),
        owner: "org".into(),
        repo: "app".into(),
    };
    state.lending.checkouts.set_for_tests(
        [(
            "/code/app".to_owned(),
            Checkout {
                origin: Some(repository("github.com")),
                upstream: None,
                branch: Some("topic".into()),
                head: Some("a".repeat(40)),
                tracked: None,
            },
        )]
        .into(),
    );
    state.lending.set_gh_hosts(vec![GhHost {
        host: "ghe.example.com".into(),
        login: "corp-me".into(),
    }]);
    sheprd_open(&state, "octocat");
    let full = home(state.clone(), token.clone()).await;
    assert_eq!(
        full["workspaces"][0]["github"],
        serde_json::json!({
            "host": "github.com", "owner": "org", "repo": "app",
            "branch": "topic", "head": "a".repeat(40),
        })
    );
    assert_eq!(
        full["github_hosts"],
        serde_json::json!([
            {"host": "github.com", "login": "octocat"},
            {"host": "ghe.example.com", "login": "corp-me"},
        ])
    );

    // A remote off GitHub names nothing.
    state.lending.checkouts.set_for_tests(
        [(
            "/code/app".to_owned(),
            Checkout {
                origin: Some(repository("gitlab.com")),
                ..Checkout::default()
            },
        )]
        .into(),
    );
    let elsewhere = home(state.clone(), token.clone()).await;
    assert!(elsewhere["workspaces"][0].get("github").is_none());
}

/// Set in the child that [`a_lent_token_never_reaches_the_log`] runs alone.
const LOG_ALONE: &str = "SHEPRD_TEST_LEND_LOG_ALONE";

/// Whether a log line has a subscriber is decided once per process, so a
/// parallel test meeting the line first, on a thread with none, can mark it
/// off for every thread. The check runs again in a process of its own.
#[test]
fn a_lent_token_never_reaches_the_log() {
    if std::env::var_os(LOG_ALONE).is_some() {
        return lent_token_log_alone();
    }
    let output = sheprd_client::new_command(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "web::tests::lend::a_lent_token_never_reaches_the_log",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(LOG_ALONE, "1")
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{report}");
    assert!(report.contains("1 passed"), "{report}");
}

fn lent_token_log_alone() {
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Log(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Log {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let log = Log::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    // While a single dispatcher is registered, tracing-core caches a
    // callsite's interest from whichever thread registers it first, so a
    // parallel test reaching the log line with no subscriber would cache
    // "never" for this thread too. A second live dispatcher makes it ask
    // every registered dispatcher instead, under its lock.
    let _other = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _default = tracing::subscriber::set_default(subscriber);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let state = lending_state(dir.path(), Lending::new(Vec::new(), gh_signed_in));
        let (_, token) = signed_in(&state, 1);
        let (status, body) = lent(&state, &token, "github.com").await;
        assert_eq!(
            (status, body["token"].as_str()),
            (StatusCode::OK, Some(GH_TOKEN))
        );
    });
    let log = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("Lent a GitHub sign-in"), "{log}");
    assert!(!log.contains(GH_TOKEN), "{log}");
}
