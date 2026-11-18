use crate::native::{
    github::{checkouts::read, hosts::Recognized, remote::Repository},
    model::WorkspaceGitHub,
};
use std::process::Stdio;

fn git(repository: &std::path::Path, args: &[&str]) {
    let status = sheprd_client::new_command("git")
        .current_dir(repository)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A fork: `origin` is `me/app`, `upstream` is `org/app`, on `topic`.
fn fork(parent: &std::path::Path) -> std::path::PathBuf {
    let repository = parent.join("fork");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    std::fs::write(repository.join("a.txt"), "one\n").unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "--quiet", "-m", "start"]);
    git(&repository, &["checkout", "--quiet", "-b", "topic"]);
    git(
        &repository,
        &["remote", "add", "origin", "git@github.com:Me/app.git"],
    );
    git(
        &repository,
        &[
            "remote",
            "add",
            "upstream",
            "https://github.com/org/app.git",
        ],
    );
    repository
}

fn named(repository: &std::path::Path) -> WorkspaceGitHub {
    WorkspaceGitHub::of(&read(repository).unwrap(), &Recognized::default()).unwrap()
}

#[test]
fn a_fork_names_origin_as_the_head_owner() {
    let dir = tempfile::tempdir().unwrap();
    let repository = fork(dir.path());
    let named = named(&repository);
    assert_eq!(
        (named.owner.as_str(), named.head_owner.as_deref()),
        ("org", Some("Me"))
    );
}

#[test]
fn a_branch_names_the_remote_it_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let repository = fork(dir.path());
    git(
        &repository,
        &[
            "remote",
            "add",
            "mine",
            "https://github.com/someone-else/app",
        ],
    );
    git(&repository, &["config", "branch.topic.remote", "mine"]);
    let checkout = read(&repository).unwrap();
    assert_eq!(
        checkout.tracked.map(|tracked| tracked.owner),
        Some("someone-else".into())
    );
    assert_eq!(
        named(&repository).head_owner.as_deref(),
        Some("someone-else")
    );

    // A branch tracking upstream itself is on the upstream's owner.
    git(&repository, &["config", "branch.topic.remote", "upstream"]);
    assert_eq!(named(&repository).head_owner, None);

    // A branch tracking a local one falls back to origin.
    git(&repository, &["config", "branch.topic.remote", "."]);
    assert_eq!(named(&repository).head_owner.as_deref(), Some("Me"));
}

#[test]
fn the_same_owner_in_any_case_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let repository = fork(dir.path());
    git(
        &repository,
        &["remote", "set-url", "upstream", "https://github.com/me/app"],
    );
    assert_eq!(named(&repository).head_owner, None);
}

#[test]
fn a_head_owner_off_the_host_or_not_a_login_is_left_out() {
    let on = |host: &str, owner: &str| {
        Some(Repository {
            host: host.into(),
            owner: owner.into(),
            repo: "app".into(),
        })
    };
    let checkout = crate::native::github::checkouts::Checkout {
        origin: on("github.com", "me"),
        upstream: on("github.com", "org"),
        branch: Some("topic".into()),
        head: None,
        tracked: on("gitlab.com", "other"),
    };
    let recognized = Recognized::default();
    assert_eq!(
        WorkspaceGitHub::of(&checkout, &recognized)
            .unwrap()
            .head_owner,
        None
    );
    let dotted = crate::native::github::checkouts::Checkout {
        tracked: on("github.com", "not.a.login"),
        ..checkout
    };
    assert_eq!(
        WorkspaceGitHub::of(&dotted, &recognized)
            .unwrap()
            .head_owner,
        None
    );
}

#[test]
fn head_owner_is_serialized_only_when_set() {
    let mut named = WorkspaceGitHub {
        host: "github.com".into(),
        owner: "org".into(),
        repo: "app".into(),
        branch: Some("topic".into()),
        head: Some("a".repeat(40)),
        head_owner: None,
    };
    assert_eq!(
        serde_json::to_value(&named).unwrap(),
        serde_json::json!({
            "host": "github.com", "owner": "org", "repo": "app",
            "branch": "topic", "head": "a".repeat(40),
        })
    );
    named.head_owner = Some("me".into());
    assert_eq!(
        serde_json::to_value(&named).unwrap(),
        serde_json::json!({
            "host": "github.com", "owner": "org", "repo": "app",
            "branch": "topic", "head": "a".repeat(40), "head_owner": "me",
        })
    );
}
