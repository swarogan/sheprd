//! The heading's button that collapses every host at once, or expands them
//! all when they all are, as the phone's Workspaces tab offers.

use crate::{
    SheprdWindow,
    config::corners,
    direction::Logical,
    i18n::fl,
    sidebar::{layout::SidebarLook, line_height, scaled},
};
use gpui::{prelude::*, *};

impl SheprdWindow {
    /// The button beside the list's heading, or `None` with fewer than two
    /// hosts headed.
    pub(in crate::sidebar) fn fold_hosts_button(
        &self,
        look: SidebarLook,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let folded = self.hosts_folded(cx)?;
        let font = &self.config.sidebar;
        let theme = &self.theme;
        let height = line_height(font) + 2. * look.density.header_padding();
        let size = scaled(font, 20.).min(height - 2.);
        let icon = scaled(font, 12.);
        let label: SharedString = if folded {
            fl!("sidebar-expand-all-hosts")
        } else {
            fl!("sidebar-collapse-all-hosts")
        }
        .into();
        let (foreground, surface) = (theme.foreground, theme.surface);
        let hint = label.clone();
        let button = div()
            .id("sidebar-fold-hosts")
            .debug_selector(|| "sidebar-fold-hosts".into());
        Some(
            // A keyboard stop, as the sidebar's other buttons are.
            self.sidebar_button(button, label)
                .flex_none()
                .size(px(size))
                // The icon's edge lines up with the rows' status column.
                .me(px((look.content_x() - (size - icon) / 2.).max(0.)))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(corners::SMALL))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme.active)))
                .tooltip(move |_, cx| {
                    cx.new(|_| crate::usage::Hint {
                        text: hint.clone(),
                        foreground,
                        surface,
                    })
                    .into()
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_all_devices(cx);
                }))
                .child(
                    svg()
                        .path(if folded {
                            "icons/expand-all.svg"
                        } else {
                            "icons/collapse-all.svg"
                        })
                        .size(px(icon))
                        .text_color(rgb(theme.muted)),
                ),
        )
    }
}
