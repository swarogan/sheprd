use crate::native::{
    github::{
        checkouts::{Checkout, read},
        hosts::Recognized,
        remote::Repository,
    },
    model::WorkspaceGitHub,
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

fn git(repository: &Path, args: &[&str]) -> String {
    let output = sheprd_client::new_command("git")
        .current_dir(repository)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .stderr(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A repository on `main` with one commit, and its commit.
fn repository(parent: &Path) -> (PathBuf, String) {
    let repository = parent.join("project");
    std::fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    std::fs::write(repository.join("a.txt"), "one\n").unwrap();
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "--quiet", "-m", "start"]);
    let head = git(&repository, &["rev-parse", "HEAD"]);
    (repository, head)
}

fn on(host: &str, owner: &str, repo: &str) -> Option<Repository> {
    Some(Repository {
        host: host.into(),
        owner: owner.into(),
        repo: repo.into(),
    })
}

#[test]
fn a_checkout_names_its_remotes_branch_and_commit() {
    let dir = tempfile::tempdir().unwrap();
    let (repository, head) = repository(dir.path());
    git(
        &repository,
        &["remote", "add", "origin", "git@github.com:me/app.git"],
    );
    assert_eq!(
        read(&repository),
        Some(Checkout {
            origin: on("github.com", "me", "app"),
            upstream: None,
            branch: Some("main".into()),
            head: Some(head),
            tracked: None,
        })
    );
}

#[test]
fn a_fork_names_its_upstream_repository() {
    let dir = tempfile::tempdir().unwrap();
    let (repository, head) = repository(dir.path());
    git(
        &repository,
        &["remote", "add", "origin", "https://github.com/me/app.git"],
    );
    git(
        &repository,
        &[
            "remote",
            "add",
            "upstream",
            "ssh://git@github.com/org/app.git",
        ],
    );
    let checkout = read(&repository).unwrap();
    assert_eq!(checkout.upstream, on("github.com", "org", "app"));
    let named = WorkspaceGitHub::of(&checkout, &Recognized::default()).unwrap();
    assert_eq!(
        named,
        WorkspaceGitHub {
            host: "github.com".into(),
            owner: "org".into(),
            repo: "app".into(),
            branch: Some("main".into()),
            head: Some(head),
            head_owner: Some("me".into()),
        }
    );
}

#[test]
fn an_upstream_off_github_leaves_origin_named() {
    let checkout = Checkout {
        origin: on("github.com", "me", "app"),
        upstream: on("gitlab.com", "org", "app"),
        branch: Some("topic".into()),
        head: None,
        tracked: None,
    };
    let named = WorkspaceGitHub::of(&checkout, &Recognized::default()).unwrap();
    assert_eq!((named.owner.as_str(), named.head), ("me", None));
}

#[test]
fn a_detached_head_names_its_commit_without_a_branch() {
    let dir = tempfile::tempdir().unwrap();
    let (repository, head) = repository(dir.path());
    git(
        &repository,
        &["remote", "add", "origin", "https://github.com/me/app"],
    );
    git(&repository, &["checkout", "--quiet", "--detach"]);
    let checkout = read(&repository).unwrap();
    assert_eq!((checkout.branch, checkout.head), (None, Some(head)));
}

#[test]
fn no_remote_or_no_repository_names_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (repository, _) = repository(dir.path());
    assert_eq!(read(&repository), None);
    git(
        &repository,
        &["remote", "add", "backup", "https://github.com/me/app"],
    );
    assert_eq!(read(&repository), None);
    let plain = dir.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    assert_eq!(read(&plain), None);
    assert_eq!(read(&dir.path().join("missing")), None);
}

#[test]
fn enterprise_hosts_are_named_only_once_recognized() {
    let checkout = Checkout {
        origin: on("ghe.example.com", "team", "app"),
        upstream: None,
        branch: Some("main".into()),
        head: None,
        tracked: None,
    };
    assert_eq!(WorkspaceGitHub::of(&checkout, &Recognized::default()), None);
    let configured = Recognized {
        configured: vec!["ghe.example.com".into()],
        gh: Vec::new(),
    };
    assert_eq!(
        WorkspaceGitHub::of(&checkout, &configured).map(|named| named.host),
        Some("ghe.example.com".into())
    );
    let ghe_com = Checkout {
        origin: on("acme.ghe.com", "team", "app"),
        ..checkout.clone()
    };
    assert!(WorkspaceGitHub::of(&ghe_com, &Recognized::default()).is_some());
    let elsewhere = Checkout {
        origin: on("gitlab.com", "team", "app"),
        ..checkout
    };
    assert_eq!(WorkspaceGitHub::of(&elsewhere, &configured), None);
}
