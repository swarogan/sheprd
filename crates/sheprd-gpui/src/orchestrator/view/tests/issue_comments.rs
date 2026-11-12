//! A Gitea or Jira issue's page lists its comments above the comment field;
//! a lookup for another issue is not shown; another provider's page has no
//! comments section. The worker does not take actions in these tests, so
//! the snapshot is set directly.
use super::*;
use crate::orchestrator::{SourceKey, view::rows::Tab};
use chrono::{TimeZone, Utc};
use gpui::TestAppContext;
use sheprd_orchestrator::{
    Error,
    comments::{Comment, Lookup},
    gitea,
};
use std::sync::Arc;

fn gitea_issue() -> Item {
    let mut issue = item(Provider::Gitea, "7", "Fix it", "2026-10-04T00:00:00Z");
    issue.key.source = SourceKey {
        provider: Provider::Gitea,
        host: "forge.example.com".into(),
        repository: "acme/app".into(),
    };
    issue
}

fn jira_issue() -> Item {
    let mut issue = item(Provider::Jira, "PROJ-1", "Fix it", "2026-10-04T00:00:00Z");
    issue.key.source = SourceKey {
        provider: Provider::Jira,
        host: "acme.atlassian.net".into(),
        repository: "assigned".into(),
    };
    issue
}

fn comment(author: &str, body: &str) -> Comment {
    Comment {
        author: author.into(),
        created_at: Some(Utc.with_ymd_and_hms(2026, 10, 5, 0, 0, 0).unwrap()),
        body: body.into(),
    }
}

/// `issue`'s page, open, with `lookup` loaded for `id` of the same source.
fn open_page<'a>(
    cx: &'a mut TestAppContext,
    issue: Item,
    lookup: Option<(&'static str, Result<Vec<Comment>, Error>)>,
) -> (
    tempfile::TempDir,
    gpui::Entity<crate::orchestrator::view::OrchestratorView>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, view, cx) = empty::open(cx);
    let key = issue.key.canonical();
    let source = issue.key.source.clone();
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.repo_mut().snapshot.items = Arc::new(vec![issue]);
            view.tab = Tab::Issues;
            view.refresh_rows();
            view.open_detail(key, cx);
            view.repo_mut().snapshot.comments = lookup.map(|(id, result)| Lookup {
                source,
                issue: id.into(),
                result: Arc::new(result),
            });
            cx.notify();
        })
    });
    cx.run_until_parked();
    (dir, view, cx)
}

fn y(cx: &mut gpui::VisualTestContext, id: &'static str) -> f32 {
    f32::from(
        cx.debug_bounds(id)
            .unwrap_or_else(|| panic!("{id}"))
            .origin
            .y,
    )
}

#[gpui::test]
fn a_gitea_page_lists_its_comments_above_the_comment_field(cx: &mut TestAppContext) {
    let comments = vec![
        comment("ada", "one line"),
        comment("me", "first\n\nsecond\n\nthird"),
    ];
    let (_dir, _view, cx) = open_page(cx, gitea_issue(), Some(("7", Ok(comments))));
    assert!(cx.debug_bounds("orchestrator-comments-loading").is_none());
    let (first, second) = (
        cx.debug_bounds("orchestrator-comment-0").unwrap(),
        cx.debug_bounds("orchestrator-comment-1").unwrap(),
    );
    assert!(first.origin.y < second.origin.y);
    // A body of several paragraphs takes more room than a line.
    assert!(second.size.height > first.size.height);
    assert!(y(cx, "orchestrator-comment-1") < y(cx, "orchestrator-gitea-comment-field"));
    assert!(y(cx, "orchestrator-comments") < y(cx, "orchestrator-comment-0"));
}

#[gpui::test]
fn the_comments_follow_the_description_and_the_field_follows_the_comments(cx: &mut TestAppContext) {
    let mut issue = jira_issue();
    issue.description = Some("Why it breaks".into());
    let comments = vec![comment("Ada", "later")];
    let (_dir, _view, cx) = open_page(cx, issue, Some(("PROJ-1", Ok(comments))));
    assert!(y(cx, "orchestrator-jira-assign") < y(cx, "orchestrator-description"));
    assert!(y(cx, "orchestrator-description") < y(cx, "orchestrator-comments"));
    assert!(y(cx, "orchestrator-comment-0") < y(cx, "orchestrator-jira-comment-field"));
}

#[gpui::test]
fn the_empty_loading_and_failed_states_say_so(cx: &mut TestAppContext) {
    let (_dir, _view, cx) = open_page(cx, gitea_issue(), Some(("7", Ok(Vec::new()))));
    assert!(cx.debug_bounds("orchestrator-comments-empty").is_some());
    assert!(cx.debug_bounds("orchestrator-comment-0").is_none());

    let (_dir, _view, cx) = open_page(cx, gitea_issue(), None);
    assert!(cx.debug_bounds("orchestrator-comments-loading").is_some());
    assert!(
        cx.debug_bounds("orchestrator-gitea-comment-field")
            .is_some()
    );

    let failed = Err(Error::from(gitea::Error::Forbidden));
    let (_dir, _view, cx) = open_page(cx, gitea_issue(), Some(("7", failed)));
    assert!(cx.debug_bounds("orchestrator-comments-error").is_some());
    assert!(cx.debug_bounds("orchestrator-comments-loading").is_none());
}

#[gpui::test]
fn a_lookup_for_another_issue_is_not_shown(cx: &mut TestAppContext) {
    let other = vec![comment("ada", "not this one")];
    let (_dir, _view, cx) = open_page(cx, gitea_issue(), Some(("8", Ok(other))));
    assert!(cx.debug_bounds("orchestrator-comment-0").is_none());
    assert!(cx.debug_bounds("orchestrator-comments-loading").is_some());
}

#[gpui::test]
fn a_github_issue_page_has_no_comments_section(cx: &mut TestAppContext) {
    let github = item(Provider::Github, "377", "Icons", "2026-10-04T00:00:00Z");
    let (_dir, _view, cx) = open_page(cx, github, None);
    assert!(cx.debug_bounds("orchestrator-detail-title").is_some());
    assert!(cx.debug_bounds("orchestrator-comments").is_none());
}
