//! A Jira issue's page offers to change its status, comment, and take the
//! issue; another provider's issue page offers none of it.
use super::*;
use crate::orchestrator::{SourceKey, view::rows::Tab};
use gpui::{Modifiers, TestAppContext};
use sheprd_orchestrator::jira::{Lookup, Transition};
use std::sync::Arc;

const CONTROLS: [&str; 3] = [
    "orchestrator-jira-comment",
    "orchestrator-jira-comment-field",
    "orchestrator-jira-assign",
];

fn jira_issue() -> Item {
    let mut issue = item(Provider::Jira, "PROJ-1", "Fix it", "2026-10-04T00:00:00Z");
    issue.key.source = SourceKey {
        provider: Provider::Jira,
        host: "acme.atlassian.net".into(),
        repository: "assigned".into(),
    };
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
fn a_jira_issue_page_has_the_three_controls_and_its_transitions(cx: &mut TestAppContext) {
    let (_dir, view, cx) = open_page(cx, jira_issue());
    for id in CONTROLS {
        assert!(cx.debug_bounds(id).is_some(), "{id}");
    }
    // The transitions are still loading, and then they are buttons.
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            let issue = jira_issue();
            view.repo_mut().snapshot.jira_transitions = Some(Lookup {
                source: issue.key.source,
                issue: issue.identifier,
                result: Arc::new(Ok(vec![Transition {
                    id: "31".into(),
                    name: "Start progress".into(),
                    to: Some("In Progress".into()),
                }])),
            });
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("orchestrator-jira-transition-31").is_some());
}

#[gpui::test]
fn a_github_issue_page_has_none_of_the_jira_controls(cx: &mut TestAppContext) {
    let github = item(Provider::Github, "377", "Icons", "2026-10-04T00:00:00Z");
    let (_dir, _view, cx) = open_page(cx, github);
    assert!(cx.debug_bounds("orchestrator-detail-title").is_some());
    for id in CONTROLS {
        assert!(cx.debug_bounds(id).is_none(), "{id}");
    }
}

#[gpui::test]
fn commenting_with_an_empty_field_sends_nothing(cx: &mut TestAppContext) {
    let (_dir, view, cx) = open_page(cx, jira_issue());
    let button = cx.debug_bounds("orchestrator-jira-comment").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!view.read(cx).jira.busy && view.read(cx).notice.is_none()));

    // With text, the same click reaches the worker: the controls wait for
    // it, or it has already answered or was too busy to take it.
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.jira
                .comment
                .update(cx, |input, cx| input.set_text_at("looks good", 10, cx));
        })
    });
    cx.simulate_click(button.center(), Modifiers::none());
    cx.update(|_, cx| assert!(view.read(cx).jira.busy || view.read(cx).notice.is_some()));
}
