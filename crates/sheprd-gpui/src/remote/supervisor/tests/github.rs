//! Lending the main GitHub account: asks are taken only while a phone could
//! be asking, the token is checked with GitHub first, and only github.com
//! is lent.

use super::*;
use crate::remote::supervisor::github::{Account, answers};
use secrecy::SecretString;
use sheprd_remote_control::control::{
    changes::AskId,
    github::{LeaseAnswer, LeaseAsk},
};
use std::sync::Mutex;

fn ask(id: u64, host: &str) -> LeaseAsk {
    LeaseAsk {
        id: AskId(id),
        host: host.into(),
    }
}

fn works(_: &SecretString) -> sheprd_github::Result<String> {
    Ok("OctoCat".into())
}

fn rejected(_: &SecretString) -> sheprd_github::Result<String> {
    Err(sheprd_github::Error::Authentication)
}

fn another_account(_: &SecretString) -> sheprd_github::Result<String> {
    Ok("someone-else".into())
}

static CHECKS: AtomicUsize = AtomicUsize::new(0);

fn counted(_: &SecretString) -> sheprd_github::Result<String> {
    CHECKS.fetch_add(1, Ordering::SeqCst);
    Ok("octocat".into())
}

static ASKED: Mutex<Vec<Request>> = Mutex::new(Vec::new());

fn daemon_with_an_ask(_: &Path, request: &Request) -> Result<Response, control::Error> {
    ASKED.lock().unwrap().push(request.clone());
    match request {
        Request::Status => Ok(Response::Status(Status {
            github_asks: 1,
            ..status(0)
        })),
        Request::GitHubLeaseAsks { .. } => Ok(Response::GitHubLeaseAsks {
            asks: vec![ask(4, "github.com")],
        }),
        _ => Ok(Response::Done),
    }
}

#[gpui::test]
fn an_ask_waiting_is_taken_and_answered_with_the_checked_token(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Supervisor::fixture(cx, on(), daemon_with_an_ask);
        Supervisor::set_github_for_tests(cx, Some(("octocat", "ghu_main")), works);
    });
    for _ in 0..2 {
        cx.update(Supervisor::tick);
        cx.run_until_parked();
    }
    let asked = ASKED.lock().unwrap().clone();
    assert!(asked.contains(&Request::GitHubLeaseAsks {
        login: Some("octocat".into())
    }));
    let lent = asked.iter().find_map(|request| match request {
        Request::GitHubLease { id, answer } => Some((*id, answer.clone())),
        _ => None,
    });
    let Some((AskId(4), LeaseAnswer::Lease(lease))) = lent else {
        panic!("no lease: {asked:?}");
    };
    assert_eq!(
        (lease.login.as_str(), lease.token.expose()),
        ("octocat", "ghu_main")
    );
}

static IDLE_ASKS: AtomicUsize = AtomicUsize::new(0);

fn idle_daemon(_: &Path, request: &Request) -> Result<Response, control::Error> {
    match request {
        Request::Status => Ok(Response::Status(status(0))),
        Request::GitHubLeaseAsks { .. } => {
            IDLE_ASKS.fetch_add(1, Ordering::SeqCst);
            Ok(Response::GitHubLeaseAsks { asks: Vec::new() })
        }
        _ => Ok(Response::Done),
    }
}

#[gpui::test]
fn with_no_phone_and_no_ask_nothing_waits_on_the_daemon(cx: &mut TestAppContext) {
    cx.update(|cx| Supervisor::fixture(cx, on(), idle_daemon));
    for _ in 0..2 {
        cx.update(Supervisor::tick);
        cx.run_until_parked();
    }
    assert_eq!(IDLE_ASKS.load(Ordering::SeqCst), 0);
}

#[test]
fn only_a_token_github_still_takes_is_lent_and_only_for_github_com() {
    let main = Some(Account::fixture("octocat", "ghu_main"));
    let asks = [ask(1, "github.com"), ask(2, "ghe.example.com")];
    let lent = answers(main.as_ref(), &asks, works);
    assert!(
        matches!(&lent[0], (AskId(1), LeaseAnswer::Lease(lease)) if lease.token.expose() == "ghu_main")
    );
    assert_eq!(lent[1], (AskId(2), LeaseAnswer::SignedOut));
    for check in [rejected as fn(&SecretString) -> _, another_account] {
        assert_eq!(
            answers(main.as_ref(), &asks[..1], check),
            [(AskId(1), LeaseAnswer::SignedOut)]
        );
    }
    assert_eq!(
        answers(None, &asks[..1], works),
        [(AskId(1), LeaseAnswer::SignedOut)]
    );
    // One check, however many asks came.
    let many = [
        ask(1, "github.com"),
        ask(2, "github.com"),
        ask(3, "github.com"),
    ];
    let lent = answers(main.as_ref(), &many, counted);
    assert_eq!(lent.len(), 3);
    assert_eq!(CHECKS.load(Ordering::SeqCst), 1);
    // No ask for github.com: GitHub is not asked.
    answers(main.as_ref(), &asks[1..], counted);
    assert_eq!(CHECKS.load(Ordering::SeqCst), 1);
}
