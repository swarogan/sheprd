use super::*;

fn existing(fixture: &Fixture) -> String {
    create(
        &fixture.host(),
        &fixture.herdr2(),
        &NewWorktree {
            branch: "pr/1250",
            base: "HEAD",
            label: None,
        },
        &never(),
    )
    .unwrap()
    .path
}

fn remote(fixture: &Fixture) {
    let remote = fixture.temp.path().join("remote.git");
    git(&fixture.repo, &["init", "--bare", remote.to_str().unwrap()]);
    git(
        &fixture.repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&fixture.repo, &["push", "origin", "main"]);
}

fn advance(fixture: &Fixture) -> String {
    fs::write(fixture.repo.join("a.txt"), "remote update").unwrap();
    git(&fixture.repo, &["commit", "-am", "advance"]);
    git(
        &fixture.repo,
        &["push", "origin", "main", "HEAD:refs/pull/1250/head"],
    );
    git(&fixture.repo, &["rev-parse", "HEAD"])
}

#[test]
fn reuse_pulls_fork_pr_ref_without_an_upstream_and_keeps_local_edits() {
    let fixture = Fixture::new();
    remote(&fixture);
    let path = existing(&fixture);
    let expected = advance(&fixture);
    fs::write(Path::new(&path).join("untracked.txt"), "keep me").unwrap();
    pull_existing(
        &fixture.host(),
        fixture.repo.to_str().unwrap(),
        &path,
        "pr/1250",
        Some(("origin", "refs/pull/1250/head")),
        &never(),
    )
    .unwrap();
    assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), expected);
    assert_eq!(
        fs::read_to_string(Path::new(&path).join("untracked.txt")).unwrap(),
        "keep me"
    );
}

#[test]
fn reuse_pulls_the_configured_upstream() {
    let fixture = Fixture::new();
    remote(&fixture);
    let path = existing(&fixture);
    git(
        Path::new(&path),
        &["branch", "--set-upstream-to=origin/main"],
    );
    let expected = advance(&fixture);
    pull_existing(
        &fixture.host(),
        fixture.repo.to_str().unwrap(),
        &path,
        "pr/1250",
        None,
        &never(),
    )
    .unwrap();
    assert_eq!(git(Path::new(&path), &["rev-parse", "HEAD"]), expected);
}

#[test]
fn reuse_does_not_stash_conflicting_edits_or_rewrite_divergent_commits() {
    let fixture = Fixture::new();
    remote(&fixture);
    let path = existing(&fixture);
    let tree = Path::new(&path);
    git(tree, &["branch", "--set-upstream-to=origin/main"]);
    git(tree, &["config", "pull.rebase", "true"]);
    git(tree, &["config", "rebase.autoStash", "true"]);
    git(tree, &["config", "merge.autoStash", "true"]);
    advance(&fixture);
    fs::write(tree.join("a.txt"), "local edits").unwrap();
    let head = git(tree, &["rev-parse", "HEAD"]);
    let pull = || {
        pull_existing(
            &fixture.host(),
            fixture.repo.to_str().unwrap(),
            &path,
            "pr/1250",
            None,
            &never(),
        )
    };
    assert!(matches!(
        pull(),
        Err(Error::Script {
            step: Step::Pull,
            ..
        })
    ));
    assert_eq!(git(tree, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read_to_string(tree.join("a.txt")).unwrap(),
        "local edits"
    );
    assert!(git(tree, &["stash", "list"]).is_empty());
    git(tree, &["commit", "-am", "local"]);
    let local = git(tree, &["rev-parse", "HEAD"]);
    assert!(pull().is_err());
    assert_eq!(git(tree, &["rev-parse", "HEAD"]), local);
}

#[test]
fn reuse_refuses_a_different_repository_or_changed_branch() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    remote(&fixture);
    let path = existing(&fixture);
    advance(&fixture);
    for (repository, branch) in [
        (other.repo.to_str().unwrap(), "pr/1250"),
        (fixture.repo.to_str().unwrap(), "another-branch"),
    ] {
        assert!(matches!(
            pull_existing(
                &fixture.host(),
                repository,
                &path,
                branch,
                Some(("origin", "refs/pull/1250/head")),
                &never()
            ),
            Err(Error::Script {
                step: Step::Pull,
                ..
            })
        ));
    }
    assert_eq!(
        fs::read_to_string(Path::new(&path).join("a.txt")).unwrap(),
        "a"
    );
}
