//! The comments of a Gitea or Jira issue on its page: the worker reads
//! them, and the comment field sits below them so a posted comment appears
//! right above it.

use super::{
    OrchestratorView,
    gitea_actions::index,
    look::{RED, ago},
    markdown::Markdown,
};
use crate::{
    i18n::{Localize, fl},
    orchestrator::{Item, Provider},
    release_notes::{self, Line},
};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};

/// The parsed lines of each comment drawn, by position, so a repaint parses
/// nothing again.
#[derive(Default)]
pub(super) struct Bodies(RefCell<Vec<Markdown>>);

impl Bodies {
    fn lines(&self, position: usize, body: &str) -> Rc<[Line]> {
        let mut all = self.0.borrow_mut();
        while all.len() <= position {
            all.push(Markdown::default());
        }
        all[position].lines(body)
    }
}

impl OrchestratorView {
    /// The open item, when it is a Gitea or Jira one with comments to show.
    fn open_commented_item(&self) -> Option<&Item> {
        self.detail
            .as_ref()
            .and_then(|detail| self.item(&detail.key))
            .filter(|item| commented(item))
    }

    /// Asks the worker for what the open Jira or Gitea item's page shows.
    pub(super) fn reload_issue_lookups(&self) {
        self.reload_jira_transitions();
        self.reload_comments();
    }

    /// Asks the worker for the open item's comments.
    pub(super) fn reload_comments(&self) {
        if let (Some(item), Some(service)) = (self.open_commented_item(), self.service()) {
            service.load_comments(item.key.source.clone(), comments_id(item));
        }
    }

    /// The comment field with its button, for the provider of `item`.
    fn comment_row(&self, item: &Item, cx: &mut Context<Self>) -> Div {
        if item.key.source.provider == Provider::Jira {
            self.jira.row(
                &self.look,
                (
                    "orchestrator-jira-comment-field",
                    "orchestrator-jira-comment",
                ),
                fl!("jira-actions-comment"),
                Self::post_jira_comment,
                cx,
            )
        } else {
            self.gitea.row(
                &self.look,
                (
                    "orchestrator-gitea-comment-field",
                    "orchestrator-gitea-comment",
                ),
                fl!("gitea-actions-comment"),
                Self::post_gitea_comment,
                cx,
            )
        }
    }

    /// The comments heading, the comments or why there are none, and the
    /// comment field.
    pub(super) fn render_comments(&self, item: &Item, cx: &mut Context<Self>) -> Div {
        let look = &self.look;
        let id = comments_id(item);
        let lookup = self
            .snapshot()
            .comments
            .as_ref()
            .filter(|found| found.source == item.key.source && found.issue == id);
        let now = chrono::Utc::now();
        let list = match lookup.map(|found| found.result.as_ref()) {
            None => look
                .muted(fl!("issue-comments-loading"))
                .debug_selector(|| "orchestrator-comments-loading".into()),
            Some(Err(error)) => div()
                .debug_selector(|| "orchestrator-comments-error".into())
                .text_size(look.small())
                .text_color(rgb(look.hue(RED)))
                .child(error.localize()),
            Some(Ok(list)) if list.is_empty() => look
                .muted(fl!("issue-comments-none"))
                .debug_selector(|| "orchestrator-comments-empty".into()),
            Some(Ok(list)) => {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(list.iter().enumerate().map(|(position, comment)| {
                        let mut parts = Vec::new();
                        if !comment.author.is_empty() {
                            parts.push(comment.author.clone());
                        }
                        if comment.created_at.is_some() {
                            parts.push(ago(comment.created_at, now));
                        }
                        let selector = format!("orchestrator-comment-{position}");
                        let lines = self.comment_bodies.lines(position, &comment.body);
                        div()
                            .id(("orchestrator-comment", position))
                            .debug_selector(move || selector.clone())
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .when(position > 0, |el| {
                                el.pt_2().border_t_1().border_color(rgb(look.theme.active))
                            })
                            .child(look.muted(parts.join(" \u{00b7} ")))
                            .child(release_notes::render(
                                "orchestrator-comment-body",
                                &lines,
                                &look.theme,
                                &look.mono,
                            ))
                    }))
            }
        };
        div()
            .debug_selector(|| "orchestrator-comments".into())
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(fl!("issue-comments-heading")),
            )
            .child(list)
            .child(self.comment_row(item, cx))
    }
}

/// Whether `item` is one whose comments the page shows.
pub(super) fn commented(item: &Item) -> bool {
    matches!(item.key.source.provider, Provider::Gitea | Provider::Jira)
}

/// What the worker reads `item`'s comments by: a Gitea number, a Jira key.
fn comments_id(item: &Item) -> String {
    if item.key.source.provider == Provider::Jira {
        item.identifier.clone()
    } else {
        index(item)
    }
}
