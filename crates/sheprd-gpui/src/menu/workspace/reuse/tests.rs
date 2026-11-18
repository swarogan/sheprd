#![allow(clippy::unwrap_used)]

use super::*;
use crate::{
    menu::{Page, WorkspaceAction},
    sidebar::layout_tests,
};
use gpui::TestAppContext;
use serde_json::json;

fn listing(branch: &str) -> crate::state::DialogResponse {
    Ok(json!({"result": {"type": "worktree_list",
        "source": {"repo_key": "/repo/.git", "repo_name": "repo", "source_workspace_id": "w3"},
        "worktrees": [{"path": "/repo/existing", "branch": branch, "label": "existing",
            "is_bare": false, "is_prunable": false, "is_detached": false,
            "open_workspace_id": "w4"}]
    }}))
}

#[gpui::test]
fn an_existing_open_checkout_is_reused_and_does_not_arm_a_new_agent(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(layout_tests::fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap()).workspaces =
                layout_tests::snapshot(7).workspaces;
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.open_workspace_menu("w3", Default::default(), window, cx);
            view.open_workspace_dialog(WorkspaceAction::NewWorktree, window, cx);
            view.finish_checkout_check(
                "pr/1250".into(),
                "already exists".into(),
                listing("pr/1250"),
                cx,
            );
            let source = view.menu.worktree.as_ref().unwrap();
            assert!(
                matches!(&source.pending, Some(Pending::Checkout(path)) if path == "/repo/existing")
            );
            assert_eq!(
                source.reuse_notice.as_deref(),
                Some("Already exists — will open it and pull its remote.")
            );
            assert!(view.landing_launch.is_none());
            let request = view.menu.creation.clone().unwrap();
            assert!(view.reuse_request_current(&request));
            let original_epoch = view.selection_epoch;
            view.selection_epoch += 1;
            assert!(!view.reuse_request_current(&request));
            view.selection_epoch = original_epoch;
            view.menu.reset();
            assert!(!view.reuse_request_current(&request));
        })
    });
}

#[gpui::test]
fn a_failed_pull_still_opens_the_existing_workspace_and_reports_the_warning(
    cx: &mut TestAppContext,
) {
    let (view, cx) = cx.add_window_view(layout_tests::fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap()).workspaces =
                layout_tests::snapshot(7).workspaces;
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.open_workspace_menu("w3", Default::default(), window, cx);
            view.open_workspace_dialog(WorkspaceAction::NewWorktree, window, cx);
            let warning = pull_notice(Err(crate::Error::NotConnected));
            let source = view.menu.worktree.as_mut().unwrap();
            source.pending = Some(Pending::Checkout("/repo/existing".into()));
            source.reuse_notice = Some(warning);
            view.apply_creation_response(
                Ok(json!({"result": {
                    "type": "worktree_opened", "workspace": {"workspace_id": "w4"}
                }})),
                window,
                cx,
            );
            assert!(view.menu.page.is_none());
            assert_eq!(
                view.pending_navigation,
                Some(crate::NavigationTarget::Workspace("w4".into()))
            );
            assert!(view.landing_launch.is_none());
            assert!(view.worktree_script.is_none());
            let notice = &view.endpoints[view.selected_endpoint]
                .toasts
                .entries
                .back()
                .unwrap()
                .1;
            assert_eq!(notice.title, "Worktree already exists");
            // Toasts strip Fluent's direction-isolation marks for display.
            assert_eq!(
                notice.body.as_deref(),
                Some("Could not pull the existing worktree's remote: Not connected to a daemon.")
            );
        });
    });
}

#[gpui::test]
fn a_slug_collision_or_failed_listing_keeps_the_original_creation_error(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(layout_tests::fixture_window);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            std::sync::Arc::make_mut(view.live.snapshot.as_mut().unwrap()).workspaces =
                layout_tests::snapshot(7).workspaces;
            view.live.status = crate::state::ConnectionStatus::Connected;
            view.open_workspace_menu("w3", Default::default(), window, cx);
            view.open_workspace_dialog(WorkspaceAction::NewWorktree, window, cx);
            for response in [
                listing("different/branch"),
                Ok(json!({"error": {"code": "unavailable"}})),
            ] {
                view.finish_checkout_check(
                    "pr/1250".into(),
                    "original refusal".into(),
                    response,
                    cx,
                );
                assert_eq!(view.menu.error.as_deref(), Some("original refusal"));
                assert!(view.menu.creation.is_none());
                assert!(view.menu.worktree.as_ref().unwrap().pending.is_none());
                assert_eq!(
                    view.menu.page,
                    Some(Page::Dialog(WorkspaceAction::NewWorktree))
                );
            }
        })
    });
}

#[test]
fn fork_pulls_use_the_remote_pr_ref_and_issues_use_the_upstream() {
    let mut item = crate::repo_items::Item {
        kind: crate::repo_items::Kind::PullRequest,
        number: 1250,
        title: "title".into(),
        url: String::new(),
        author: String::new(),
        head: Some("feature".into()),
        fork_owner: Some("contributor".into()),
        remote: crate::pull_request::Remote::Origin,
        draft: false,
    };
    assert_eq!(
        pull_remote(Some(&Pending::Item(item.clone()))),
        Some(("origin".into(), "refs/pull/1250/head".into()))
    );
    item.fork_owner = None;
    assert_eq!(
        pull_remote(Some(&Pending::Item(item.clone()))),
        Some(("origin".into(), "refs/heads/feature".into()))
    );
    item.head = None;
    assert_eq!(pull_remote(Some(&Pending::Item(item))), None);
}
