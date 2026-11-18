//! The one-time offer to move `Herdr.app` to `Sheprd.app`: the move itself
//! for a standalone app, or the commands that reinstall a Homebrew one.

use super::{Flash, SheprdWindow};
use crate::app_rename::{AppRename, Choice};
use crate::i18n::{Localize, fl};
use gpui::{ClipboardItem, Context, Window};
use sheprd_updater::Legacy;

impl SheprdWindow {
    /// Offers the move once, when this is the first window ready to show a
    /// dialog. Runs every tick; asking claims the question app-wide.
    pub(crate) fn offer_app_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ready = self.menu.page.is_none() && self.active && self.live.snapshot.is_some();
        if !ready || !AppRename::take_ask(window.window_handle().window_id(), cx) {
            return;
        }
        if self.open_menu(window, cx) {
            self.menu.page = Some(crate::menu::Page::AppRename);
            cx.notify();
        }
    }

    /// The dialog's main action: the move, or copying the Homebrew commands.
    pub(crate) fn confirm_app_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match AppRename::bundle(cx).cloned() {
            Some(Legacy::Standalone(_)) => self.rename_app(window, cx),
            Some(Legacy::Homebrew(homebrew)) => {
                cx.write_to_clipboard(ClipboardItem::new_string(homebrew.commands()));
                AppRename::choose(Choice::Copied, cx).detach();
                self.close_app_rename(window, cx);
                let flash = Flash::success(fl!("window-notices-rename-commands-copied"));
                self.show_flash(flash, cx);
            }
            None => {}
        }
    }

    fn close_app_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.page == Some(crate::menu::Page::AppRename) {
            self.dismiss_menu(window, cx);
        }
    }

    /// Moves the app, then quits so it starts again from its new name. The
    /// terminals stay with the daemon, as on any quit.
    fn rename_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Legacy::Standalone(bundle)) = AppRename::bundle(cx).cloned() else {
            return;
        };
        // Saved whatever the outcome, so the app never asks again.
        let saved = AppRename::choose(Choice::Renamed, cx);
        self.close_app_rename(window, cx);
        let rename = cx
            .background_executor()
            .spawn(async move { bundle.rename_and_relaunch() });
        cx.spawn(async move |this, cx| {
            let renamed = rename.await;
            saved.await;
            match renamed {
                Ok(()) => cx.update(|cx| cx.quit()),
                Err(error) => {
                    tracing::warn!(%error, "Cannot rename the app");
                    this.update(cx, |this, cx| {
                        let flash = Flash::warning(fl!(
                            "window-notices-rename-failed",
                            error = error.localize()
                        ));
                        this.show_flash(flash, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub(crate) fn render_app_rename_offer(&self, cx: &mut Context<Self>) -> gpui::Div {
        use crate::fonts::StyledFont;
        use gpui::{prelude::*, *};
        let theme = &self.theme;
        let button = |id: &'static str, label: String, primary: bool| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .p(px(8.))
                .rounded(px(crate::config::corners::CONTROL))
                .when(primary, |button| button.bg(rgb(theme.active)))
                .when(!primary, |button| button.hover(|s| s.bg(rgb(theme.active))))
                .cursor_pointer()
                .child(label)
        };
        let text = |text: String| div().p(px(8.)).child(text);
        let muted = |text: String| div().p(px(8.)).text_color(rgb(theme.muted)).child(text);
        let mut offer = div().debug_selector(|| "app-rename-offer".into());
        let Some(legacy) = AppRename::bundle(cx) else {
            return offer;
        };
        let from = legacy.path().display().to_string();
        let confirm = match legacy {
            Legacy::Homebrew(homebrew) => {
                offer = offer
                    .child(text(fl!("window-notices-rename-homebrew-title")))
                    .child(text(fl!(
                        "window-notices-rename-homebrew-body",
                        from = from
                    )))
                    .child(
                        div()
                            .debug_selector(|| "app-rename-commands".into())
                            .m(px(8.))
                            .p(px(8.))
                            .rounded(px(crate::config::corners::CONTROL))
                            .bg(rgb(theme.active))
                            .text_font(&self.config.terminal)
                            .children(homebrew.commands().lines().map(|line| {
                                // i18n: data
                                div().child(line.to_owned())
                            })),
                    )
                    .child(muted(fl!("window-notices-rename-homebrew-detail")));
                fl!("window-notices-rename-homebrew-copy")
            }
            Legacy::Standalone(bundle) => {
                offer = offer
                    .child(text(fl!("window-notices-rename-offer-title")))
                    .child(text(fl!(
                        "window-notices-rename-offer-body",
                        from = from,
                        to = bundle.renamed().display().to_string()
                    )))
                    .child(muted(fl!("window-notices-rename-offer-detail")));
                fl!("window-notices-rename-offer-confirm")
            }
        };
        offer.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .p(px(8.))
                .child(
                    button("app-rename-confirm", confirm, true).on_click(cx.listener(
                        |this, _, window, cx| {
                            cx.stop_propagation();
                            this.confirm_app_rename(window, cx);
                        },
                    )),
                )
                .child(
                    button(
                        "app-rename-decline",
                        fl!("window-notices-rename-offer-decline"),
                        false,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.dismiss_menu(window, cx);
                    })),
                ),
        )
    }
}
