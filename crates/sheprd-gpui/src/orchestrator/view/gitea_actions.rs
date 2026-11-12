//! What a person can do on a Gitea issue's page: close or reopen it, comment,
//! and take it. A pull request's page takes only a comment. The worker does
//! the writing; the controls wait while one of them is in flight.

use super::{OrchestratorView, write_controls::Press};
use crate::{
    i18n::fl,
    orchestrator::{Action, Done, Item, Notice, Provider},
};
use gpui::{prelude::*, *};
use sheprd_orchestrator::gitea::write::Change;

/// The number the Gitea API knows `item` by: its key, without a pull
/// request's `pr/`.
pub(super) fn index(item: &Item) -> String {
    let id = item.key.native_id.as_str();
    id.strip_prefix("pr/").unwrap_or(id).to_owned()
}

impl OrchestratorView {
    /// The open item, when it is a Gitea issue or pull request.
    fn open_gitea_item(&self) -> Option<&Item> {
        self.detail
            .as_ref()
            .and_then(|detail| self.item(&detail.key))
            .filter(|item| item.key.source.provider == Provider::Gitea)
    }

    /// Takes a finished action's outcome: the controls work again, and a
    /// posted comment leaves its field.
    pub(super) fn gitea_noticed(&mut self, notice: &Notice, cx: &mut Context<Self>) {
        self.gitea.busy = false;
        if matches!(notice.outcome, Ok(Done::GiteaCommented)) {
            self.gitea.comment.update(cx, |input, cx| input.clear(cx));
            self.reload_comments();
        }
    }

    /// Sends `change` unless another is in flight.
    fn gitea_change(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.gitea.busy {
            return;
        }
        self.gitea.busy = self.act(Action::Gitea(change));
        cx.notify();
    }

    /// Closes the open issue, or reopens it when it is closed.
    fn toggle_gitea_state(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.open_gitea_item() else {
            return;
        };
        let change = Change::SetState {
            source: item.key.source.clone(),
            index: index(item),
            closed: !item.state.eq_ignore_ascii_case("closed"),
        };
        self.gitea_change(change, cx);
    }

    /// Takes the open issue.
    fn assign_gitea_issue(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.open_gitea_item() else {
            return;
        };
        let change = Change::Assign {
            source: item.key.source.clone(),
            index: index(item),
        };
        self.gitea_change(change, cx);
    }

    /// Posts the comment field's text on the open item; an empty one sends
    /// nothing.
    pub(super) fn post_gitea_comment(&mut self, cx: &mut Context<Self>) {
        let text = self.gitea.comment.read(cx).text().trim().to_owned();
        let Some(item) = self.open_gitea_item().filter(|_| !text.is_empty()) else {
            return;
        };
        let change = Change::Comment {
            source: item.key.source.clone(),
            index: index(item),
            text,
        };
        self.gitea_change(change, cx);
    }

    /// Enter in the comment field posts it, Escape leaves it.
    pub(super) fn gitea_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.gitea.press(key, window, cx) {
            Some(Press::Post) => self.post_gitea_comment(cx),
            Some(Press::Leave) => window.focus(&self.focus, cx),
            None => return false,
        }
        true
    }

    /// The close or reopen and Assign to me buttons of an issue.
    pub(super) fn render_gitea_actions(&self, item: &Item, cx: &mut Context<Self>) -> Div {
        let look = &self.look;
        let busy = self.gitea.busy;
        let state = if item.state.eq_ignore_ascii_case("closed") {
            fl!("gitea-actions-reopen")
        } else {
            fl!("gitea-actions-close")
        };
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                look.button("orchestrator-gitea-state", state, false)
                    .debug_selector(|| "orchestrator-gitea-state".into())
                    .when(busy, |el| el.opacity(0.5))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_gitea_state(cx))),
            )
            .child(
                look.button(
                    "orchestrator-gitea-assign",
                    fl!("gitea-actions-assign"),
                    false,
                )
                .debug_selector(|| "orchestrator-gitea-assign".into())
                .when(busy, |el| el.opacity(0.5))
                .on_click(cx.listener(|this, _, _, cx| this.assign_gitea_issue(cx))),
            )
    }
}
