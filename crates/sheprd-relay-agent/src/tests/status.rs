//! What `status` reports, with and without a running agent, and the state
//! directory shared by commands that run at once.

use super::*;
use crate::{
    keys::Keys,
    status::{Live, Report, STALE},
};
use std::time::SystemTime;

#[test]
fn status_makes_the_keys_so_enrolling_can_read_the_route_first() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    let first = Report::read(dir.path(), now).unwrap();
    let again = Report::read(dir.path(), now).unwrap();
    assert_eq!(first.route, again.route);
    assert_eq!(first.identity, again.identity);
    assert!(!first.running);
    assert_eq!(first.authorized, 0);
}

#[test]
fn the_route_status_reports_is_the_one_the_agent_listens_on() {
    let dir = tempfile::tempdir().unwrap();
    let report = Report::read(dir.path(), SystemTime::now()).unwrap();
    let agent = Agent::load(Options {
        forward: "127.0.0.1:22".parse().unwrap(),
        relays: Vec::new(),
        state_dir: dir.path().to_owned(),
    })
    .unwrap();
    assert_eq!(report.route, agent.route().to_string());
    assert_eq!(report.identity, agent.keys.identity());
}

#[test]
fn a_live_status_shows_the_relays_until_it_goes_stale() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    let mut live = Live::default();
    live.relays.insert(
        "https://relay.example.org/".to_owned(),
        RelayState::Listening,
    );
    live.save(dir.path(), now).unwrap();
    let report = Report::read(dir.path(), now).unwrap();
    assert!(report.running);
    assert_eq!(report.relays, live.relays);
    let later = now + STALE + Duration::from_secs(1);
    let report = Report::read(dir.path(), later).unwrap();
    assert!(!report.running);
    assert!(report.relays.is_empty());
}

#[test]
fn a_status_file_that_is_not_one_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(crate::status::FILE), b"{").unwrap();
    assert!(matches!(
        Report::read(dir.path(), SystemTime::now()),
        Err(crate::Error::StatusFormat { .. })
    ));
}

#[test]
fn the_report_reads_back_as_json() {
    let dir = tempfile::tempdir().unwrap();
    authorize(dir.path(), &[&Desktop::new(0x31)]);
    let report = Report::read(dir.path(), SystemTime::now()).unwrap();
    let json = serde_json::to_string(&report).unwrap();
    let back: Report = serde_json::from_str(&json).unwrap();
    assert_eq!(back, report);
    assert_eq!(back.authorized, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_agent_removes_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let options = Options {
        forward: "127.0.0.1:22".parse().unwrap(),
        relays: Vec::new(),
        state_dir: dir.path().to_owned(),
    };
    let agent = Agent::load(options).unwrap();
    let (stop, stopped) = watch::channel(false);
    let running = tokio::spawn(Arc::clone(&agent).run(stopped));
    let path = dir.path().join(crate::status::FILE);
    for _ in 0..100 {
        if path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(path.exists(), "the running agent wrote no status");
    stop.send(true).unwrap();
    running.await.unwrap();
    assert!(!path.exists());
}

#[test]
fn callers_racing_on_a_fresh_directory_share_one_set_of_keys() {
    let dir = tempfile::tempdir().unwrap();
    let routes: Vec<String> = std::thread::scope(|scope| {
        let callers: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| Keys::load_or_create(dir.path()).unwrap().route()))
            .collect();
        callers
            .into_iter()
            .map(|caller| caller.join().unwrap().to_string())
            .collect()
    });
    let saved = Keys::load_or_create(dir.path())
        .unwrap()
        .route()
        .to_string();
    assert!(routes.iter().all(|route| *route == saved), "{routes:?}");
}

#[test]
fn concurrent_changes_to_the_desktops_are_never_lost() {
    let dir = tempfile::tempdir().unwrap();
    let kept = Desktop::new(0x40);
    let revoked = Desktop::new(0x41);
    authorize(dir.path(), &[&revoked]);
    let dir = dir.path();
    let (kept, revoked) = (&kept, &revoked);
    std::thread::scope(|scope| {
        for seed in 0x50..0x58 {
            scope.spawn(move || {
                Authorized::update(dir, |authorized| {
                    authorized.insert(Desktop::new(seed).entry()).unwrap();
                    Ok(true)
                })
                .unwrap();
            });
        }
        scope.spawn(move || {
            Authorized::update(dir, |authorized| {
                authorized.insert(kept.entry()).unwrap();
                Ok(true)
            })
            .unwrap();
        });
        scope.spawn(move || {
            Authorized::update(dir, |authorized| {
                Ok(authorized.remove(&revoked.entry().dial))
            })
            .unwrap();
        });
    });
    let authorized = Authorized::load(dir).unwrap();
    assert_eq!(authorized.entries().len(), 9);
    assert!(authorized.admits(&kept.entry().tunnel));
    assert!(!authorized.admits(&revoked.entry().tunnel));
}

#[test]
fn a_second_agent_on_the_same_state_directory_refuses_to_start() {
    let dir = tempfile::tempdir().unwrap();
    let options = || Options {
        forward: "127.0.0.1:22".parse().unwrap(),
        relays: Vec::new(),
        state_dir: dir.path().to_owned(),
    };
    let first = Agent::load(options()).unwrap();
    assert!(matches!(
        Agent::load(options()),
        Err(crate::Error::AlreadyRunning { .. })
    ));
    drop(first);
    Agent::load(options()).unwrap();
}
