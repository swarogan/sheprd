//! Closing a workspace, or deleting its worktree, from the phone.

use super::*;
use crate::{
    hub::{ApiMethod, Workspace},
    web::workspace::{Action, herdr_call, needs_archive, wsl_archive_script},
};
use sheprd_client::protocol::ClientShellWorktree;

fn workspace(id: &str, cwd: &str, linked: Option<bool>) -> Workspace {
    Workspace {
        id: id.into(),
        label: id.into(),
        branch: None,
        cwd: cwd.into(),
        worktree: linked.map(|linked| ClientShellWorktree {
            key: "repo".into(),
            label: "repo".into(),
            is_linked_worktree: linked,
        }),
        session: None,
        chats: Vec::new(),
        terminals: Vec::new(),
    }
}

/// A computer with a main checkout (`main`), a linked worktree (`linked`)
/// in `checkout`, and a workspace outside Git (`plain`).
fn with_checkouts(dir: &std::path::Path, checkout: &std::path::Path) -> Shared {
    let nested = checkout.join("src");
    std::fs::create_dir_all(&nested).unwrap();
    let initialized = sheprd_client::new_command("git")
        .arg("-C")
        .arg(checkout)
        .args(["init", "-q"])
        .status()
        .unwrap();
    assert!(initialized.success());
    let checkout = nested.to_string_lossy();
    let layout = Layout {
        connected: true,
        boot: "b".into(),
        working: 0,
        workspaces: vec![
            workspace("main", "/code/app", Some(false)),
            workspace("linked", &checkout, Some(true)),
            workspace("plain", "", None),
        ],
    };
    let state = Arc::try_unwrap(state(dir)).ok().unwrap();
    Arc::new(State {
        hub: Hub::offline(layout),
        ..state
    })
}

#[test]
fn herdr_closes_the_workspace_alone_and_never_forces_a_deletion() {
    let (method, params) = herdr_call(Action::Close, "w1");
    assert_eq!(method.name(), "workspace.close");
    assert_eq!(
        params,
        json!({ "workspace_id": "w1", "close_group": false })
    );
    let (method, params) = herdr_call(Action::RemoveWorktree, "w1");
    assert_eq!(method, ApiMethod::WorktreeRemove);
    assert_eq!(method.name(), "worktree.remove");
    assert_eq!(
        params,
        json!({ "workspace_id": "w1", "force": false, "trust_repository": false })
    );
}

#[test]
fn an_archive_script_or_an_unreadable_script_file_leaves_deletion_to_sheprd() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!needs_archive(dir.path()), "no scripts at all");
    std::fs::create_dir(dir.path().join(".herdr")).unwrap();
    let file = dir.path().join(".herdr/worktree.toml");
    std::fs::write(&file, "[scripts]\nsetup = \"make\"\narchive = \"  \"\n").unwrap();
    assert!(
        !needs_archive(dir.path()),
        "a blank archive script runs nothing"
    );
    std::fs::write(&file, "[scripts]\narchive = \"./save.sh\"\n").unwrap();
    assert!(needs_archive(dir.path()));
    std::fs::write(&file, "not toml [").unwrap();
    assert!(needs_archive(dir.path()));
    std::fs::write(&file, "#".repeat(64 * 1024 + 1)).unwrap();
    assert!(needs_archive(dir.path()), "too large to read");
}

#[test]
fn wsl_archive_script_output_fails_closed_and_reads_the_checkout_root() {
    assert!(!wsl_archive_script(b"missing\n"));
    assert!(!wsl_archive_script(
        b"present\n[scripts]\nsetup = \"make\"\n"
    ));
    assert!(wsl_archive_script(
        b"present\n[scripts]\narchive = \"./save.sh\"\n"
    ));
    assert!(wsl_archive_script(b"unexpected\n"));
    assert!(wsl_archive_script(
        format!("present\n{}", "#".repeat(64 * 1024 + 1)).as_bytes()
    ));
}

#[tokio::test]
async fn closing_needs_a_session_and_a_known_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let checkout = tempfile::tempdir().unwrap();
    let state = with_checkouts(dir.path(), checkout.path());
    let close = |workspace: &str| post(&format!("/api/v2/workspaces/{workspace}/close"), json!({}));
    assert_eq!(
        send(&state, close("main")).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, token) = signed_in(&state, 7);
    let authorized = |workspace: &str| terminal::authorized(close(workspace), &token);
    assert_eq!(
        send(&state, authorized("w9")).await.0,
        StatusCode::NOT_FOUND
    );
    // It reaches Herdr, which this test has none of.
    let (status, _, body) = send(&state, authorized("main")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}

#[tokio::test]
async fn only_a_linked_worktree_without_an_archive_script_is_deleted_here() {
    let dir = tempfile::tempdir().unwrap();
    let checkout = tempfile::tempdir().unwrap();
    let state = with_checkouts(dir.path(), checkout.path());
    let (_, token) = signed_in(&state, 7);
    let remove = |workspace: &str| {
        terminal::authorized(
            post(
                &format!("/api/v2/workspaces/{workspace}/remove-worktree"),
                json!({}),
            ),
            &token,
        )
    };
    for workspace in ["main", "plain"] {
        let (status, _, body) = send(&state, remove(workspace)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{workspace}: {body}");
    }
    let (status, _, body) = send(&state, remove("linked")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    std::fs::create_dir(checkout.path().join(".herdr")).unwrap();
    std::fs::write(
        checkout.path().join(".herdr/worktree.toml"),
        "[scripts]\narchive = \"./save.sh\"\n",
    )
    .unwrap();
    let (status, _, body) = send(&state, remove("linked")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("archive script"), "{body}");
}
