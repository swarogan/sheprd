//! A Gitea issue's page offers to close or reopen it, comment, and take it;
//! its pull request's page only a comment; another provider's issue page
//! none of it.
use super::*;
use crate::orchestrator::{SourceKey, view::rows::Tab};
use gpui::{Modifiers, TestAppContext};
use sheprd_orchestrator::PullRequest;
use std::sync::Arc;

const COMMENT: [&str; 2] = [
    "orchestrator-gitea-comment",
    "orchestrator-gitea-comment-field",
];
const ISSUE_ONLY: [&str; 2] = ["orchestrator-gitea-state", "orchestrator-gitea-assign"];

fn gitea_issue(state: &str) -> Item {
    let mut issue = item(Provider::Gitea, "7", "Fix it", "2026-10-04T00:00:00Z");
    issue.key.source = SourceKey {
        provider: Provider::Gitea,
        host: "forge.example.com".into(),
        repository: "acme/app".into(),
    };
    issue.state = state.into();
    issue
}

fn open_page(
    cx: &mut TestAppContext,
    issue: Item,
) -> (
    tempfile::TempDir,
    gpui::Entity<crate::orchestrator::view::OrchestratorView>,
    &mut gpui::VisualTestContext,
) {
    let (dir, view, cx) = empty::open(cx);
    let key = issue.key.canonical();
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.repo_mut().snapshot.items = Arc::new(vec![issue]);
            view.tab = Tab::Issues;
            view.refresh_rows();
            view.open_detail(key, cx);
        })
    });
    cx.run_until_parked();
    (dir, view, cx)
}

#[gpui::test]
fn a_gitea_issue_page_has_its_controls(cx: &mut TestAppContext) {
    let (_dir, _view, cx) = open_page(cx, gitea_issue("open"));
    for id in COMMENT.into_iter().chain(ISSUE_ONLY) {
        assert!(cx.debug_bounds(id).is_some(), "{id}");
    }
}

#[gpui::test]
fn the_state_button_follows_the_issue_state(cx: &mut TestAppContext) {
    let (_dir, view, cx) = open_page(cx, gitea_issue("open"));
    cx.update(|_, cx| {
        assert_eq!(crate::i18n::fl!("gitea-actions-close"), "Close issue");
        assert!(!view.read(cx).gitea.busy);
    });
    let (_dir, view, cx) = open_page(cx, gitea_issue("closed"));
    assert_eq!(crate::i18n::fl!("gitea-actions-reopen"), "Reopen issue");
    // Clicking it sends the write, which a closed issue's page makes a reopening.
    let button = cx.debug_bounds("orchestrator-gitea-state").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    cx.update(|_, cx| assert!(view.read(cx).gitea.busy));
}

#[gpui::test]
fn a_gitea_pull_request_page_has_only_the_comment(cx: &mut TestAppContext) {
    let mut pull = gitea_issue("open");
    pull.key.native_id = "pr/8".into();
    pull.pull_request = Some(PullRequest {
        number: 8,
        additions: None,
        deletions: None,
        base_ref: "main".into(),
        head_ref: "feature".into(),
        base_sha: String::new(),
        head_sha: String::new(),
        head_repository: None,
    });
    let (_dir, _view, cx) = open_page(cx, pull);
    for id in COMMENT {
        assert!(cx.debug_bounds(id).is_some(), "{id}");
    }
    for id in ISSUE_ONLY {
        assert!(cx.debug_bounds(id).is_none(), "{id}");
    }
}

#[gpui::test]
fn a_github_issue_page_has_none_of_the_gitea_controls(cx: &mut TestAppContext) {
    let github = item(Provider::Github, "377", "Icons", "2026-10-04T00:00:00Z");
    let (_dir, _view, cx) = open_page(cx, github);
    assert!(cx.debug_bounds("orchestrator-detail-title").is_some());
    for id in COMMENT.into_iter().chain(ISSUE_ONLY) {
        assert!(cx.debug_bounds(id).is_none(), "{id}");
    }
}

#[gpui::test]
fn commenting_with_an_empty_field_sends_nothing(cx: &mut TestAppContext) {
    let (_dir, view, cx) = open_page(cx, gitea_issue("open"));
    let button = cx.debug_bounds("orchestrator-gitea-comment").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!view.read(cx).gitea.busy));

    // Whitespace alone is as empty.
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.gitea
                .comment
                .update(cx, |input, cx| input.set_text_at("   ", 3, cx));
        })
    });
    cx.simulate_click(button.center(), Modifiers::none());
    cx.update(|_, cx| assert!(!view.read(cx).gitea.busy));

    // With text, the same click sends it and the controls wait.
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.gitea
                .comment
                .update(cx, |input, cx| input.set_text_at("looks good", 10, cx));
        })
    });
    cx.simulate_click(button.center(), Modifiers::none());
    cx.update(|_, cx| assert!(view.read(cx).gitea.busy));
}
