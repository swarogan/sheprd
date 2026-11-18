//! The approval step's switch, Share GitHub sign-in with this phone
//! (`control::github`): on by default, naming the account it lends, and
//! saying what turning it off later does and does not undo.

use super::*;
use crate::toggles::switch;

impl SheprdWindow {
    pub(super) fn toggle_pairing_github(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.menu.remote_pairing.as_mut() else {
            return;
        };
        let share = !dialog.pairing.share_github();
        dialog.pairing.set_share_github(share);
        cx.notify();
    }

    /// The switch under the matching code, usable until the user answers.
    pub(super) fn pairing_share_github(
        &self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let dialog = self.menu.remote_pairing.as_ref()?;
        let theme = &self.theme;
        let on = dialog.pairing.share_github();
        let label = fl!("remote-github-share");
        let account = match Supervisor::github_login(cx) {
            Some(login) => fl!("remote-github-share-account", login = login),
            None => fl!("remote-github-share-gh"),
        };
        let toggle = div()
            .id("remote-pair-github")
            .debug_selector(|| "remote-pair-github".into())
            .role(Role::Switch)
            .aria_label(SharedString::from(label.clone()))
            .aria_toggled(if on { Toggled::True } else { Toggled::False })
            .flex()
            .items_start()
            .gap(px(8.))
            .px(px(4.))
            .py(px(3.))
            .rounded(px(crate::config::corners::CONTROL))
            .when(enabled, |row| {
                row.cursor_pointer()
                    .hover(|row| row.bg(rgb(theme.active)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_pairing_github(cx);
                    }))
            })
            .when(!enabled, |row| row.opacity(0.5))
            .child(switch(theme, 16., on))
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(label)
                    .child(div().text_color(rgb(theme.subtext())).child(account))
                    .child(
                        div()
                            .text_color(rgb(theme.subtext()))
                            .child(fl!("remote-github-share-note")),
                    ),
            );
        Some(
            div()
                .debug_selector(|| "remote-pair-share-github".into())
                .child(toggle),
        )
    }
}
