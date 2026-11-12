//! Every repository has one Jira source per saved site, and none without.
use super::*;
use crate::{
    ItemKey,
    jira::saved::{Login, Saved},
};

fn saved(hosts: &[&str]) -> Arc<Saved> {
    let mut saved = Saved::default();
    for host in hosts {
        let login = Login {
            cloud_id: None,
            email: "me@example.com".into(),
            token: "token".into(),
        };
        saved.set(host, Some(login)).unwrap();
    }
    Arc::new(saved)
}

fn jira(host: &str) -> SourceKey {
    jira::source_key(host)
}

fn listed(worker: &Worker) -> Vec<SourceKey> {
    worker
        .snapshot
        .sources
        .iter()
        .map(|s| s.key.clone())
        .collect()
}

#[test]
fn a_repository_gains_a_jira_source_per_saved_site_beside_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let (_service, mut worker, _host) = opened(dir.path());
    let info = worker.snapshot.repo.clone().unwrap();
    let own = sources(&info);
    assert_eq!(own.len(), 1);

    worker.jira_saved = Some(saved(&["b.example.com", "a.atlassian.net"]));
    let all = worker.sources_of(&info);
    // The remote's first, then each site by hostname; every one is read.
    assert_eq!(
        all,
        [
            own[0].clone(),
            (jira("a.atlassian.net"), true),
            (jira("b.example.com"), true)
        ]
    );
    assert_eq!(
        jira("a.atlassian.net").canonical(),
        "jira:a.atlassian.net:assigned"
    );
}

#[test]
fn without_a_saved_site_there_is_no_jira_source() {
    let dir = tempfile::tempdir().unwrap();
    let (_service, mut worker, _host) = opened(dir.path());
    worker.jira_saved = Some(Arc::default());
    let info = worker.snapshot.repo.clone().unwrap();
    assert_eq!(worker.sources_of(&info), sources(&info));
    assert!(
        listed(&worker)
            .iter()
            .all(|key| key.provider != Provider::Jira)
    );
}

#[test]
fn a_site_saved_later_is_listed_and_the_other_sources_keep_their_state() {
    let dir = tempfile::tempdir().unwrap();
    let (_service, mut worker, _host) = opened(dir.path());
    worker.jira_saved = Some(Arc::default());
    worker.recognize(Vec::new);
    let own = listed(&worker);
    let synced = SyncState::Synced {
        at: Utc::now(),
        count: 4,
    };
    worker.set_state(&own[0], synced);

    // Refresh forgets what was read; the saved record then holds a site.
    worker.jira_saved = Some(saved(&["a.atlassian.net"]));
    worker.recognize(Vec::new);
    let statuses = &worker.snapshot.sources;
    assert_eq!(statuses.len(), own.len() + 1);
    assert!(matches!(
        statuses[0].state,
        SyncState::Synced { count: 4, .. }
    ));
    let site = statuses.last().unwrap();
    assert_eq!(site.key, jira("a.atlassian.net"));
    assert!(matches!(site.state, SyncState::Syncing { count: None }));

    // Forgetting it drops the source again.
    worker.jira_saved = Some(Arc::default());
    worker.recognize(Vec::new);
    assert_eq!(listed(&worker), own);
}

#[test]
fn issues_of_a_forgotten_site_are_not_shown_and_nothing_else_is_dropped() {
    let item = |source: SourceKey, id: &str| Item {
        key: ItemKey {
            source,
            native_id: id.into(),
        },
        ..Item::stand_in(ItemKey {
            source: jira("x"),
            native_id: id.into(),
        })
    };
    let github = SourceKey {
        provider: Provider::Github,
        host: "github.com".into(),
        repository: "o/r".into(),
    };
    let all = vec![
        item(jira("kept.atlassian.net"), "A-1"),
        item(jira("gone.atlassian.net"), "B-1"),
        item(github.clone(), "7"),
    ];
    // Before the sites are read, what was listed stays.
    let mut items = all.clone();
    sources::keep_saved_jira(&mut items, None);
    assert_eq!(items, all);

    sources::keep_saved_jira(&mut items, Some(&saved(&["Kept.atlassian.net"])));
    let left: Vec<_> = items.iter().map(|item| item.key.canonical()).collect();
    assert_eq!(
        left,
        [
            "jira:kept.atlassian.net:assigned:A-1",
            "github:github.com:o/r:7"
        ]
    );
}

#[test]
fn a_site_that_is_not_saved_asks_to_sign_in_instead_of_failing() {
    let dir = tempfile::tempdir().unwrap();
    let (_service, mut worker, _host) = opened(dir.path());
    worker.jira_saved = Some(Arc::default());
    let error = worker
        .sync_jira(&jira("gone.atlassian.net"), true)
        .unwrap_err();
    assert!(
        matches!(&error, Error::Jira(source) if source.needs_sign_in()),
        "{error}"
    );
    // It waits for the user as the other providers' refusals do.
    assert!(retry_wait(&error, None) > Duration::from_secs(300));
}
