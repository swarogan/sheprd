//! A remote whose host has a saved Gitea address is read as Gitea from the
//! next sync on, ahead of GitLab, without reopening the view.
use super::*;

#[test]
fn a_saved_address_makes_the_remote_a_gitea_source_before_glab_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let (_service, mut worker, _host) = opened(dir.path());
    let source = |worker: &Worker| worker.snapshot.sources[0].clone();
    assert_eq!(source(&worker).key.provider, Provider::Github);

    let mut saved = gitea::saved::Saved::default();
    saved
        .set("http://github.acme.com:3000", Some("token".into()))
        .unwrap();
    worker.gitea_saved = Some(Arc::new(saved));
    // glab claims the host too; the saved Gitea address wins and glab is not read.
    worker.recognize(|| unreachable!());
    let status = source(&worker);
    let expected = SourceKey {
        provider: Provider::Gitea,
        host: "github.acme.com".into(),
        repository: "group/app".into(),
    };
    assert_eq!(status.key, expected);
    assert!(matches!(status.state, SyncState::Syncing { .. }));
}

#[test]
fn without_a_saved_address_the_remote_is_left_for_glab() {
    let dir = tempfile::tempdir().unwrap();
    let (_service, mut worker, _host) = opened(dir.path());
    worker.gitea_saved = Some(Arc::default());
    worker.recognize(Vec::new);
    assert_eq!(worker.snapshot.sources[0].key.provider, Provider::Github);
}
