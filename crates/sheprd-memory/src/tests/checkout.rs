use crate::checkout::branch_of;

#[test]
fn head_names_a_branch_or_none() {
    assert_eq!(
        branch_of("ref: refs/heads/feat/x\n").as_deref(),
        Some("feat/x")
    );
    assert_eq!(branch_of("ref: refs/heads/\n"), None);
    assert_eq!(
        branch_of("0123456789abcdef0123456789abcdef01234567\n"),
        None
    );
}

#[cfg(unix)]
#[test]
fn every_worktree_finds_the_same_repository_and_its_own_branch() {
    use crate::{Checkout, Error};

    let root = tempfile::tempdir().unwrap();
    let git = |dir: &std::path::Path, args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    let main = root.path().join("main");
    std::fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    git(
        &main,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "i",
        ],
    );
    let linked = root.path().join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            linked.to_str().unwrap(),
            "-b",
            "feat/x",
        ],
    );

    let first = Checkout::find(&main).unwrap();
    let second = Checkout::find(&linked.join(".")).unwrap();
    assert_eq!(first.common_dir, second.common_dir);
    assert_ne!(first.git_dir, second.git_dir);
    assert_eq!(first.branch().as_deref(), Some("main"));
    assert_eq!(second.branch().as_deref(), Some("feat/x"));
    assert!(matches!(
        Checkout::find(root.path()),
        Err(Error::NotInCheckout)
    ));
}
