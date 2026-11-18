use crate::{
    NavigationTarget, SheprdWindow,
    browser::{Scope, Store, TabId},
};
use gpui::{App, Context, Window};
use sheprd_backend::TerminalId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RecentCorePane {
    Tab { tab: TabId, terminal: TerminalId },
    Shell { terminal: TerminalId },
}

#[derive(Clone, Copy)]
pub(crate) enum RecentCoreNavigation {
    Local,
    Herdr,
}

struct Destination {
    index: usize,
    scope: Scope,
    workspace: String,
    navigation: RecentCoreNavigation,
}

impl SheprdWindow {
    fn recent_core_destination(&self, pane: &RecentCorePane, cx: &App) -> Option<Destination> {
        let (scope, workspace) = match pane {
            RecentCorePane::Tab { tab, terminal } => {
                if !self
                    .core_terminals
                    .tabs
                    .get(tab)
                    .is_some_and(|shown| shown.panes.contains(terminal))
                {
                    return None;
                }
                let tab = cx.try_global::<Store>().and_then(|store| store.get(*tab))?;
                (tab.scope.clone(), tab.workspace_id.clone())
            }
            RecentCorePane::Shell { terminal } => {
                if self
                    .core_fallback_view()
                    .is_none_or(|view| view.read(cx).terminal() != terminal)
                {
                    return None;
                }
                (
                    crate::browser::scope(&self.endpoints[0]),
                    super::WORKSPACE.into(),
                )
            }
        };
        let index = self
            .endpoints
            .iter()
            .position(|endpoint| crate::browser::scope(endpoint) == scope)?;
        if !self.can_switch_endpoint(index) {
            return None;
        }
        let current = self.browser_key().as_ref() == Some(&(scope.clone(), workspace.clone()));
        let kept = self
            .core_terminals
            .session_of(&scope)
            .is_some_and(|session| session.workspace(&workspace).is_some());
        if !current && !kept {
            let live = if index == self.selected_endpoint {
                &self.live
            } else {
                &self.endpoints[index].live
            };
            let endpoint = &self.endpoints[index];
            if !live.supports_surface
                || !live.status.is_connected()
                || endpoint
                    .connection
                    .handle
                    .as_ref()
                    .is_none_or(|handle| handle.is_disconnected())
                || live.snapshot.as_ref().is_none_or(|snapshot| {
                    !snapshot
                        .workspaces
                        .iter()
                        .any(|known| known.workspace_id == workspace)
                })
            {
                return None;
            }
        }
        Some(Destination {
            index,
            scope,
            workspace,
            navigation: if current || kept {
                RecentCoreNavigation::Local
            } else {
                RecentCoreNavigation::Herdr
            },
        })
    }

    pub(crate) fn recent_core_panes(&self, cx: &App) -> Vec<(RecentCorePane, bool)> {
        let mut panes = Vec::new();
        if let Some(store) = cx.try_global::<Store>() {
            for (tab, shown) in &self.core_terminals.tabs {
                let Some(stored) = store.get(*tab) else {
                    continue;
                };
                if !self
                    .endpoints
                    .iter()
                    .any(|endpoint| crate::browser::scope(endpoint) == stored.scope)
                {
                    continue;
                }
                panes.extend(shown.panes.terminals().into_iter().map(|terminal| {
                    let pane = RecentCorePane::Tab {
                        tab: *tab,
                        terminal: terminal.clone(),
                    };
                    let available = self.recent_core_destination(&pane, cx).is_some()
                        && self
                            .core_terminals
                            .views
                            .get(&(*tab, terminal.clone()))
                            .is_some_and(|view| !view.read(cx).is_stale());
                    (pane, available)
                }));
            }
        }
        if let Some(view) = self.core_fallback_view() {
            let pane = RecentCorePane::Shell {
                terminal: view.read(cx).terminal().clone(),
            };
            let available =
                !view.read(cx).is_stale() && self.recent_core_destination(&pane, cx).is_some();
            panes.push((pane, available));
        }
        panes
    }

    pub(crate) fn recent_core_focus(
        &self,
        window: Option<&Window>,
        cx: &App,
    ) -> Option<RecentCorePane> {
        if let Some(window) = window
            && let Some(view) = self.core_fallback_view()
            && view.read(cx).focus_handle().contains_focused(window, cx)
            && self
                .browser_key()
                .is_some_and(|(_, workspace)| workspace == super::WORKSPACE)
        {
            return Some(RecentCorePane::Shell {
                terminal: view.read(cx).terminal().clone(),
            });
        }
        let focused = window
            .and_then(|window| self.core_command_pane(window, cx))
            .filter(|(tab, _)| {
                self.group_slots()
                    .iter()
                    .any(|slot| self.group_shown(slot.id, cx) == crate::browser::Shown::Page(*tab))
            });
        if let Some((tab, terminal)) = focused.or_else(|| self.shown_core_pane(cx)) {
            return Some(RecentCorePane::Tab { tab, terminal });
        }
        if self
            .browser_key()
            .is_some_and(|(_, workspace)| workspace == super::WORKSPACE)
            && self.active_group().is_none_or(|group| {
                matches!(self.group_shown(group, cx), crate::browser::Shown::Terminal)
            })
            && let Some(view) = self.core_fallback_view()
        {
            return Some(RecentCorePane::Shell {
                terminal: view.read(cx).terminal().clone(),
            });
        }
        None
    }

    pub(crate) fn recent_core_composing(&self, window: &Window, cx: &App) -> bool {
        self.core_terminals
            .views
            .values()
            .chain(self.core_fallback_view().iter())
            .any(|view| {
                let view = view.read(cx);
                view.focus_handle().contains_focused(window, cx) && view.composing()
            })
    }

    /// Move the owning workspace first. A Herdr-owned workspace can require
    /// its normal surface fence; its core pane is shown after that settles.
    pub(crate) fn navigate_recent_core(
        &mut self,
        pane: &RecentCorePane,
        cx: &mut Context<Self>,
    ) -> Option<RecentCoreNavigation> {
        let Destination {
            index,
            scope,
            workspace,
            navigation,
        } = self.recent_core_destination(pane, cx)?;
        if self.browser_key().as_ref() == Some(&(scope, workspace.clone())) {
            return Some(RecentCoreNavigation::Local);
        }
        let queued = if index == self.selected_endpoint {
            self.navigate(NavigationTarget::Workspace(&workspace), cx)
        } else {
            let endpoint = self.endpoints[index].id.clone();
            self.navigate_endpoint(&endpoint, NavigationTarget::Workspace(&workspace), cx)
        };
        queued.then_some(navigation)
    }

    pub(crate) fn show_recent_core(
        &mut self,
        pane: &RecentCorePane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(destination) = self.recent_core_destination(pane, cx) else {
            return false;
        };
        if self.browser_key().as_ref() != Some(&(destination.scope, destination.workspace)) {
            return false;
        }
        self.core_terminals.reveal = false;
        self.core_terminals.focus_wanted = None;
        match pane {
            RecentCorePane::Tab { tab, terminal } => {
                self.show_core_pane(*tab, terminal, window, cx)
            }
            RecentCorePane::Shell { .. } => {
                self.ensure_layout();
                if let Some(group) = self.primary_group().or_else(|| self.active_group()) {
                    if let Some(layout) = self.ensure_layout() {
                        layout.follow_terminal(group);
                    }
                    self.activate_group(group, window, cx);
                }
                if let Some(view) = self.core_fallback_view() {
                    window.focus(&view.read(cx).focus_handle().clone(), cx);
                }
            }
        }
        true
    }
}
