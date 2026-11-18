//! Ctrl-Tab over the bounded window-local history of terminal panes.

mod order;

use super::SheprdWindow;
use crate::{
    core_terminals::recent::RecentCoreNavigation,
    endpoint::{Endpoint, Step},
    i18n::fl,
    navigation::{NavigationTarget, OwnedNavigationTarget},
    state::LiveState,
};
use gpui::{App, Context, ModifiersChangedEvent, Window};
pub(crate) use order::RecentPanes;
use order::{PaneKey, Projection};
use std::sync::{Arc, Mutex};

/// Identity only. SurfaceActivation remains the owner of completion evidence.
struct SelectionFence {
    endpoint: String,
    inbox: Arc<Mutex<LiveState>>,
    activation: Option<String>,
    deferred: Option<OwnedNavigationTarget>,
}

impl SheprdWindow {
    pub(super) fn switch_recent_pane(
        &mut self,
        step: Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observe_recent_focus(window, cx);
        if !self.marked.is_empty()
            || self.recent_core_composing(window, cx)
            || self.recent_panes.pending()
        {
            return;
        }
        let current = self.focused_recent_pane(Some(window), cx);
        let selected = &self.endpoints[self.selected_endpoint];
        let unavailable = selected.outage().is_some()
            || selected
                .connection
                .handle
                .as_ref()
                .is_none_or(|handle| handle.is_disconnected());
        if !unavailable
            && current
                .as_ref()
                .is_some_and(|key| matches!(key, PaneKey::Herdr { .. }))
            && !self.live.supports_surface
        {
            self.local_error = Some(fl!("window-notices-recent-panes-needs-surface"));
            cx.notify();
            return;
        }
        let Some((index, key)) =
            self.recent_panes
                .peek(step, &self.projections(), current.as_ref())
        else {
            return;
        };
        let (went, waits_for_herdr) = match &key {
            PaneKey::Herdr { endpoint, pane } => {
                if !self.input_ready() && !unavailable {
                    return;
                }
                let target = NavigationTarget::Pane(pane.as_str());
                (
                    if *endpoint == self.endpoints[self.selected_endpoint].id {
                        self.navigate(target, cx)
                    } else {
                        self.navigate_endpoint(endpoint, target, cx)
                    },
                    true,
                )
            }
            PaneKey::Core(pane) => match self.navigate_recent_core(pane, cx) {
                Some(RecentCoreNavigation::Local) => (true, false),
                Some(RecentCoreNavigation::Herdr) => (true, true),
                None => (false, false),
            },
        };
        if !went {
            return;
        }
        self.recent_panes.begin(index, window.modifiers());
        let endpoint = &self.endpoints[self.selected_endpoint];
        if waits_for_herdr {
            self.recent_panes.navigation = Some(SelectionFence {
                endpoint: endpoint.id.clone(),
                inbox: endpoint.connection.inbox.clone(),
                activation: endpoint
                    .surface_requested()
                    .then(|| self.live.activation.as_ref().map(|a| a.request.clone()))
                    .flatten(),
                deferred: self.pending_navigation.clone(),
            });
        }
        match key {
            PaneKey::Herdr { .. } => {
                self.reveal_recent_herdr();
                window.focus(&self.focus, cx);
            }
            PaneKey::Core(pane) if self.recent_panes.navigation.is_none() => {
                if self.show_recent_core(&pane, window, cx) {
                    self.observe_recent_focus(window, cx);
                } else {
                    self.recent_panes.cancel();
                }
            }
            PaneKey::Core(_) => {}
        }
    }

    pub(super) fn recent_pane_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.observe_recent_focus(window, cx);
        self.recent_panes.release(event.modifiers);
    }

    /// Read local keyboard focus at command/release boundaries, rather than
    /// waiting for a later terminal poll to record a click on a split pane.
    pub(crate) fn observe_recent_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.check_recent_navigation();
        self.recent_panes.sync_core(self.recent_core_panes(cx));
        let projected = projections(&self.endpoints, self.selected_endpoint, &self.live);
        self.recent_panes.observe(&projected, None);
        self.reveal_completed_recent_herdr();
        if self.recent_navigation_complete()
            && let Some(PaneKey::Core(pane)) = self.recent_panes.selected().cloned()
        {
            if self
                .recent_core_panes(cx)
                .iter()
                .any(|(known, available)| *known == pane && *available)
            {
                if self.show_recent_core(&pane, window, cx) {
                    self.recent_panes.navigation = None;
                } else {
                    self.recent_panes.cancel();
                }
            } else {
                self.recent_panes.cancel();
            }
        }
        let focused = self.focused_recent_pane(Some(window), cx);
        self.record_recent_focus(focused, cx);
    }

    pub(crate) fn observe_recent_panes(&mut self, cx: &App) {
        self.check_recent_navigation();
        self.reveal_completed_recent_herdr();
        let focused = self.focused_recent_pane(None, cx);
        self.record_recent_focus(focused, cx);
    }

    fn record_recent_focus(&mut self, focused: Option<PaneKey>, cx: &App) {
        self.recent_panes.sync_core(self.recent_core_panes(cx));
        let settled = if self.recent_panes.pending() {
            self.recent_panes.navigation.is_none()
                || self.recent_navigation_complete()
                    && matches!(self.recent_panes.selected(), Some(PaneKey::Herdr { .. }))
        } else {
            self.pending_navigation.is_none()
                && self.pending_toast.is_none()
                && !self.live.activation_pending()
                || matches!(focused, Some(PaneKey::Core(_)))
        };
        if settled && self.recent_panes.pending() {
            self.recent_panes.navigation = None;
        }
        let focused = settled.then_some(focused).flatten();
        let projections = projections(&self.endpoints, self.selected_endpoint, &self.live);
        self.recent_panes.observe(&projections, focused);
    }

    fn focused_recent_pane(&self, window: Option<&Window>, cx: &App) -> Option<PaneKey> {
        if let Some(pane) = self.recent_core_focus(window, cx) {
            return Some(PaneKey::Core(pane));
        }
        if self.kept_in_front().is_some()
            || self.active_group().is_some_and(|group| {
                !matches!(self.group_shown(group, cx), crate::browser::Shown::Terminal)
            })
        {
            return None;
        }
        let pane = self.live.snapshot.as_ref()?.focused_pane_id.clone()?;
        Some(PaneKey::Herdr {
            endpoint: self.endpoints[self.selected_endpoint].id.clone(),
            pane,
        })
    }

    fn recent_navigation_complete(&self) -> bool {
        let Some(fence) = &self.recent_panes.navigation else {
            return false;
        };
        fence.deferred.is_none()
            && self.pending_navigation.is_none()
            && self.pending_toast.is_none()
            && self.live.activation.as_ref().is_some_and(|activation| {
                Some(&activation.request) == fence.activation.as_ref()
                    && !activation.failed
                    && activation.active
                    && activation.revision.is_some()
                    && activation.focus.is_none()
            })
    }

    fn check_recent_navigation(&mut self) {
        let Some(fence) = &mut self.recent_panes.navigation else {
            return;
        };
        let endpoint = &self.endpoints[self.selected_endpoint];
        let same_connection = endpoint.id == fence.endpoint
            && Arc::ptr_eq(&endpoint.connection.inbox, &fence.inbox)
            && endpoint.enabled
            && endpoint.outage().is_none()
            && endpoint
                .connection
                .handle
                .as_ref()
                .is_some_and(|handle| !handle.is_disconnected());
        let activation = self.live.activation.as_ref().map(|a| &a.request);
        if !same_connection
            || fence.activation.is_some() && activation != fence.activation.as_ref()
            || fence.deferred.is_some() && self.pending_navigation != fence.deferred
        {
            self.recent_panes.cancel();
        }
    }

    fn reveal_completed_recent_herdr(&mut self) {
        if self.recent_navigation_complete() {
            self.reveal_recent_herdr();
        }
    }

    fn reveal_recent_herdr(&mut self) {
        let Some(PaneKey::Herdr { endpoint, pane }) = self.recent_panes.selected() else {
            return;
        };
        if self.endpoints[self.selected_endpoint].id != *endpoint {
            return;
        }
        let Some(pane) = self
            .live
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.panes.iter().find(|known| known.pane_id == *pane))
        else {
            return;
        };
        if self
            .browser_key()
            .is_none_or(|(_, workspace)| workspace != pane.workspace_id)
        {
            return;
        }
        let tab = pane.tab_id.clone();
        if let Some(layout) = self.ensure_layout() {
            let group = layout.owner(&crate::browser::Pick::Herdr(tab.clone()));
            layout.choose(group, crate::browser::Pick::Herdr(tab));
        }
    }

    pub(crate) fn recent_activation_started(&mut self) {
        if let Some(fence) = &mut self.recent_panes.navigation
            && fence.deferred.is_some()
            && fence.activation.is_none()
        {
            fence.activation = self.live.activation.as_ref().map(|a| a.request.clone());
        }
    }

    pub(crate) fn recent_navigation_dispatched(
        &mut self,
        target: &OwnedNavigationTarget,
        queued: bool,
    ) {
        let Some(fence) = self
            .recent_panes
            .navigation
            .as_mut()
            .filter(|fence| fence.deferred.is_some())
        else {
            return;
        };
        if !queued || fence.deferred.as_ref() != Some(target) {
            self.recent_panes.cancel();
            return;
        }
        fence.deferred = None;
        fence.activation = self.live.activation.as_ref().map(|a| a.request.clone());
    }

    pub(crate) fn cancel_recent_navigation(&mut self) {
        if self.recent_panes.navigation.is_some() {
            self.recent_panes.cancel();
        }
    }

    fn projections(&self) -> Vec<Projection<'_>> {
        projections(&self.endpoints, self.selected_endpoint, &self.live)
    }
}

fn projections<'a>(
    endpoints: &'a [Endpoint],
    selected: usize,
    live: &'a LiveState,
) -> Vec<Projection<'a>> {
    endpoints
        .iter()
        .enumerate()
        .map(|(index, endpoint)| {
            let live = if index == selected {
                live
            } else {
                &endpoint.live
            };
            Projection {
                endpoint: &endpoint.id,
                available: live.supports_surface,
                snapshot: live.snapshot.as_deref().filter(|_| {
                    endpoint.enabled
                        && live.status.is_connected()
                        && endpoint
                            .connection
                            .handle
                            .as_ref()
                            .is_some_and(|handle| !handle.is_disconnected())
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
