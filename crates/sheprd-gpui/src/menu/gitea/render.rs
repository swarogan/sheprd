//! Drawing the account menu's Gitea page: a short help line, a card for each
//! saved instance, and the form that checks and saves an address and token.

use super::{InstanceStatus, Lookup, is_plain_http, keys::Action, status_line};
use crate::i18n::fl;
use crate::{SheprdWindow, menu::Page, search_input::SearchInput};
use gpui::{prelude::*, *};

/// The Gitea mark, drawn in one color as the other providers' are.
pub(crate) fn gitea_mark(color: u32) -> Svg {
    svg()
        .path("icons/gitea.svg")
        .flex_none()
        .text_color(rgb(color))
}

impl SheprdWindow {
    /// An instance's card: the account's initial and name, the address, how
    /// the token checked, and Forget.
    fn render_gitea_instance(
        &self,
        index: usize,
        status: &InstanceStatus,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = &self.theme;
        let account = match &status.lookup {
            Lookup::Signed(account) => Some(account),
            _ => None,
        };
        let initial = account
            .and_then(|account| account.name.chars().next())
            .map(|initial| initial.to_uppercase().to_string());
        let title = account.map_or_else(|| status.host.clone(), |account| account.name.clone());
        let ready = !self.menu.gitea.working;
        let host = status.host.clone();
        let selected = self.menu.gitea.is_selected(&Action::Forget(host.clone()));
        let forget = self
            .gitlab_button(
                format!("gitea-forget-{index}"),
                fl!("gitea-menu-forget"),
                false,
                ready,
            )
            .when(selected, |button| self.selected_look(button))
            .when(ready, |button| {
                button.on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.forget_gitea(host.clone(), cx);
                }))
            });
        div()
            .debug_selector(move || format!("gitea-instance-{index}"))
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(crate::config::corners::PANEL))
            .border_1()
            .border_color(rgb(theme.active))
            .bg(rgb(theme.background))
            .child(
                div()
                    .debug_selector(move || format!("gitea-avatar-{index}"))
                    .flex_none()
                    .size(px(36.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(theme.active))
                    .child(match initial {
                        Some(initial) => div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(initial)
                            .into_any_element(),
                        None => gitea_mark(theme.muted).size(px(20.)).into_any_element(),
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(rgb(theme.muted))
                            .child(status.address.clone()),
                    )
                    .child(
                        div()
                            .debug_selector(move || format!("gitea-status-{index}"))
                            .text_color(rgb(theme.subtext()))
                            .child(status_line(status)),
                    ),
            )
            .child(forget)
    }

    pub(in crate::menu) fn render_gitea_page(&self, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let gitea = &self.menu.gitea;
        let ready = !gitea.working;
        let field = |label: String, selector: &'static str, input: &Option<Entity<SearchInput>>| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(label)
                .children(input.as_ref().map(|input| {
                    div()
                        .debug_selector(move || selector.into())
                        .child(input.clone())
                }))
        };
        let plain = is_plain_http(&self.typed_gitea_address(cx));
        let body = div()
            .id("gitea-body")
            .debug_selector(|| "gitea-body".into())
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&gitea.scroll)
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_color(rgb(theme.subtext()))
                    .child(fl!("gitea-menu-help")),
            )
            .children(gitea.problem.clone().map(|problem| {
                div()
                    .debug_selector(|| "gitea-problem".into())
                    .text_color(rgb(theme.ink(theme.palette[1])))
                    .child(problem)
            }))
            .children(
                gitea
                    .instances
                    .iter()
                    .enumerate()
                    .map(|(index, status)| self.render_gitea_instance(index, status, cx)),
            )
            .child(
                field(
                    fl!("gitea-menu-address"),
                    "gitea-address-input",
                    &gitea.address,
                )
                .when(plain, |column| {
                    column.child(
                        div()
                            .debug_selector(|| "gitea-plain-http".into())
                            .text_color(rgb(theme.ink(theme.palette[1])))
                            .child(fl!("gitea-menu-plain-http")),
                    )
                }),
            )
            .child(field(
                fl!("gitea-menu-token"),
                "gitea-token-input",
                &gitea.token,
            ))
            .child(
                div()
                    .id("gitea-create-token")
                    .debug_selector(|| "gitea-create-token".into())
                    .role(Role::Link)
                    .aria_label(fl!("gitea-menu-create-token"))
                    .cursor_pointer()
                    .text_color(crate::menu::accent(theme))
                    .hover(|style| style.underline())
                    .when(gitea.is_selected(&Action::CreateToken), |link| {
                        link.underline()
                    })
                    .child(fl!("gitea-menu-create-token"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_gitea_token_page(cx);
                    })),
            )
            .children(gitea.message.clone().map(|message| {
                div()
                    .debug_selector(|| "gitea-message".into())
                    .p(px(10.))
                    .rounded(px(crate::config::corners::CONTROL))
                    .bg(rgb(theme.background))
                    .child(message)
            }));
        let footer = div()
            .flex_none()
            .p(px(12.))
            .border_t_1()
            .border_color(rgb(theme.active))
            .flex()
            .justify_end()
            .gap(px(8.))
            .child(
                self.gitlab_button("gitea-save", fl!("gitea-menu-check-and-save"), true, ready)
                    .when(gitea.is_selected(&Action::Save), |button| {
                        self.selected_look(button)
                    })
                    .when(ready, |button| {
                        button.on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.save_gitea(cx);
                        }))
                    }),
            );
        div()
            .flex()
            .flex_col()
            .w_full()
            .min_h_0()
            .child(self.render_account_tabs(Page::Gitea, cx))
            .child(body)
            .child(footer)
    }
}
