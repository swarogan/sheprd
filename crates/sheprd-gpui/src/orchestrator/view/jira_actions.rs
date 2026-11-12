//! What a person can do on a Jira issue's page: pick one of the transitions
//! Jira offers, comment, and take the issue. The worker does the writing;
//! the controls wait while one of them is in flight.

use super::{OrchestratorView, look::RED};
use crate::{
    config::corners,
    i18n::{Localize, fl},
    orchestrator::{Action, Done, Item, Notice, Provider},
    search_input::SearchInput,
};
use gpui::{prelude::*, *};
use sheprd_orchestrator::jira::Change;

/// The comment field and whether a write is under way.
pub(super) struct Controls {
    pub(super) comment: Entity<SearchInput>,
    pub(super) busy: bool,
}

impl Controls {
    pub(super) fn new(look: &super::Look, cx: &mut Context<OrchestratorView>) -> Self {
        let comment = cx.new(|cx| {
            let mut input = SearchInput::new(cx);
            input.set_placeholder_with(|| fl!("jira-actions-comment-placeholder"), cx);
            input.set_appearance(look.ui.clone(), look.theme.clone(), cx);
            input
        });
        Self {
            comment,
            busy: false,
        }
    }
}

impl OrchestratorView {
    /// The open item, when it is a Jira issue.
    fn open_jira_issue(&self) -> Option<&Item> {
        self.detail
            .as_ref()
            .and_then(|detail| self.item(&detail.key))
            .filter(|item| item.key.source.provider == Provider::Jira)
    }

    /// Asks the worker for the open Jira issue's transitions.
    pub(super) fn reload_jira_transitions(&self) {
        if let (Some(item), Some(service)) = (self.open_jira_issue(), self.service()) {
            service.load_jira_transitions(item.key.source.clone(), item.identifier.clone());
        }
    }

    /// Takes a finished action's outcome: the controls work again, a posted
    /// comment leaves its field, and the transitions are read again.
    pub(super) fn jira_noticed(&mut self, notice: &Notice, cx: &mut Context<Self>) {
        self.jira.busy = false;
        match notice.outcome {
            Ok(Done::JiraCommented) => self.jira.comment.update(cx, |input, cx| input.clear(cx)),
            Ok(Done::JiraTransitioned) => self.reload_jira_transitions(),
            _ => {}
        }
    }

    /// Sends `change` unless another is in flight.
    fn jira_change(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.jira.busy {
            return;
        }
        self.jira.busy = self.act(Action::Jira(change));
        cx.notify();
    }

    /// Posts the comment field's text on the open issue; an empty one sends
    /// nothing.
    pub(super) fn post_jira_comment(&mut self, cx: &mut Context<Self>) {
        let text = self.jira.comment.read(cx).text().trim().to_owned();
        let Some(item) = self.open_jira_issue().filter(|_| !text.is_empty()) else {
            return;
        };
        let change = Change::Comment {
            source: item.key.source.clone(),
            issue: item.identifier.clone(),
            text,
        };
        self.jira_change(change, cx);
    }

    /// Enter in the comment field posts it, Escape leaves it.
    pub(super) fn jira_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.jira.comment.read(cx).focus.is_focused(window) {
            return false;
        }
        match key {
            "enter" => self.post_jira_comment(cx),
            "escape" => window.focus(&self.focus, cx),
            _ => return false,
        }
        true
    }

    /// The transitions, the comment field, and Assign to me of a Jira issue.
    pub(super) fn render_jira_actions(&self, item: &Item, cx: &mut Context<Self>) -> Div {
        let look = &self.look;
        let theme = &look.theme;
        let busy = self.jira.busy;
        let source = item.key.source.clone();
        let issue = item.identifier.clone();
        let lookup = self
            .snapshot()
            .jira_transitions
            .as_ref()
            .filter(|found| found.source == source && found.issue == issue);
        let transitions = match lookup.map(|found| found.result.as_ref()) {
            None => look.muted(fl!("jira-actions-loading")).into_any_element(),
            Some(Err(error)) => div()
                .text_size(look.small())
                .text_color(rgb(look.hue(RED)))
                .child(error.localize())
                .into_any_element(),
            Some(Ok(list)) => div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(list.iter().map(|transition| {
                    let (id, selector) = (
                        transition.id.clone(),
                        format!("orchestrator-jira-transition-{}", transition.id),
                    );
                    let (source, issue) = (source.clone(), item.identifier.clone());
                    look.button(
                        SharedString::from(selector.clone()),
                        transition.name.clone(),
                        false,
                    )
                    .debug_selector(move || selector.clone())
                    .when(busy, |el| el.opacity(0.5))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let change = Change::Transition {
                            source: source.clone(),
                            issue: issue.clone(),
                            id: id.clone(),
                        };
                        this.jira_change(change, cx);
                    }))
                }))
                .into_any_element(),
        };
        let assign = Change::Assign { source, issue };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(transitions)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .debug_selector(|| "orchestrator-jira-comment-field".into())
                            .flex_1()
                            .min_w(px(160.))
                            .px_2()
                            .py(px(4.))
                            .rounded(px(corners::CONTROL))
                            .border_1()
                            .border_color(rgb(theme.active))
                            .child(self.jira.comment.clone()),
                    )
                    .child(
                        look.button(
                            "orchestrator-jira-comment",
                            fl!("jira-actions-comment"),
                            true,
                        )
                        .debug_selector(|| "orchestrator-jira-comment".into())
                        .when(busy, |el| el.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| this.post_jira_comment(cx))),
                    ),
            )
            .child(
                div().flex().child(
                    look.button(
                        "orchestrator-jira-assign",
                        fl!("jira-actions-assign"),
                        false,
                    )
                    .debug_selector(|| "orchestrator-jira-assign".into())
                    .when(busy, |el| el.opacity(0.5))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.jira_change(assign.clone(), cx);
                    })),
                ),
            )
    }
}
