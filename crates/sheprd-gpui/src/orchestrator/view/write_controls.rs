//! What the Jira and Gitea issue pages share: a comment field, whether a
//! write is in flight, and the comment row both draw.

use super::{Look, OrchestratorView};
use crate::{config::corners, search_input::SearchInput};
use gpui::{prelude::*, *};

/// The comment field and whether a write is under way.
pub(super) struct Controls {
    pub(super) comment: Entity<SearchInput>,
    pub(super) busy: bool,
}

/// What a key does in a focused comment field.
pub(super) enum Press {
    Post,
    Leave,
}

impl Controls {
    pub(super) fn new(
        look: &Look,
        placeholder: impl Fn() -> String + 'static,
        cx: &mut Context<OrchestratorView>,
    ) -> Self {
        let comment = cx.new(|cx| {
            let mut input = SearchInput::new(cx);
            input.set_placeholder_with(placeholder, cx);
            input.set_appearance(look.ui.clone(), look.theme.clone(), cx);
            input
        });
        Self {
            comment,
            busy: false,
        }
    }

    /// What `key` does, when the comment field has the focus: Enter posts,
    /// Escape leaves.
    pub(super) fn press(&self, key: &str, window: &Window, cx: &App) -> Option<Press> {
        if !self.comment.read(cx).focus.is_focused(window) {
            return None;
        }
        match key {
            "enter" => Some(Press::Post),
            "escape" => Some(Press::Leave),
            _ => None,
        }
    }

    /// The field with its button, which `post` answers.
    pub(super) fn row(
        &self,
        look: &Look,
        (field_id, button_id): (&'static str, &'static str),
        label: String,
        post: fn(&mut OrchestratorView, &mut Context<OrchestratorView>),
        cx: &mut Context<OrchestratorView>,
    ) -> Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .debug_selector(|| field_id.into())
                    .flex_1()
                    .min_w(px(160.))
                    .px_2()
                    .py(px(4.))
                    .rounded(px(corners::CONTROL))
                    .border_1()
                    .border_color(rgb(look.theme.active))
                    .child(self.comment.clone()),
            )
            .child(
                look.button(button_id, label, true)
                    .debug_selector(|| button_id.into())
                    .when(self.busy, |el| el.opacity(0.5))
                    .on_click(cx.listener(move |this, _, _, cx| post(this, cx))),
            )
    }
}
