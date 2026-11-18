use super::SelectionFence;
use crate::{core_terminals::recent::RecentCorePane, endpoint::Step};
use gpui::Modifiers;
use sheprd_client::protocol::ClientShellSnapshot;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PaneKey {
    Herdr { endpoint: String, pane: String },
    Core(RecentCorePane),
}

#[derive(Clone, Copy)]
pub(crate) struct Projection<'a> {
    pub(crate) endpoint: &'a str,
    pub(crate) snapshot: Option<&'a ClientShellSnapshot>,
    pub(crate) available: bool,
}

#[derive(Default)]
pub(crate) struct RecentPanes {
    boots: HashMap<String, String>,
    core: HashMap<RecentCorePane, bool>,
    cycle: Option<Cycle>,
    order: VecDeque<PaneKey>,
    pub(super) navigation: Option<SelectionFence>,
}

struct Cycle {
    cursor: PaneKey,
    hold: Option<Modifiers>,
    pending: bool,
    confirmed: Option<PaneKey>,
    observed: Option<PaneKey>,
}

impl RecentPanes {
    pub(crate) const LIMIT: usize = 100;

    pub(super) fn sync_core(&mut self, panes: impl IntoIterator<Item = (RecentCorePane, bool)>) {
        self.core = panes.into_iter().collect();
    }

    pub(super) fn observe(&mut self, projections: &[Projection], focused: Option<PaneKey>) {
        for projection in projections {
            let Some(snapshot) = projection.snapshot else {
                continue;
            };
            if self
                .boots
                .get(projection.endpoint)
                .is_some_and(|boot| *boot != snapshot.boot_id)
            {
                self.forget(projection.endpoint);
            }
            self.boots
                .insert(projection.endpoint.to_owned(), snapshot.boot_id.clone());
        }
        while let Some(gone) = self
            .boots
            .keys()
            .find(|endpoint| !projections.iter().any(|p| p.endpoint == *endpoint))
        {
            let gone = gone.clone();
            self.forget(&gone);
        }
        let retained = |key: &PaneKey| match key {
            PaneKey::Herdr { endpoint, pane } => projections
                .iter()
                .find(|p| p.endpoint == endpoint)
                .is_some_and(|p| p.snapshot.is_none_or(|snapshot| open(snapshot, pane))),
            PaneKey::Core(pane) => self.core.contains_key(pane),
        };
        if let Some(cycle) = &mut self.cycle {
            if cycle.confirmed.as_ref().is_some_and(|key| !retained(key)) {
                cycle.confirmed = None;
            }
            if cycle.observed.as_ref().is_some_and(|key| !retained(key)) {
                cycle.observed = None;
            }
        }
        if self
            .cycle
            .as_ref()
            .is_some_and(|cycle| !retained(&cycle.cursor))
        {
            self.cancel();
        }
        self.order.retain(|key| match key {
            PaneKey::Herdr { endpoint, pane } => projections
                .iter()
                .find(|p| p.endpoint == endpoint)
                .is_some_and(|p| p.snapshot.is_none_or(|snapshot| open(snapshot, pane))),
            PaneKey::Core(pane) => self.core.contains_key(pane),
        });
        let Some(key) = focused else {
            return;
        };
        if !self.reachable(projections, &key) {
            return;
        }
        if let Some(cycle) = &mut self.cycle {
            if cycle.pending {
                cycle.confirmed = Some(cycle.cursor.clone());
                cycle.pending = false;
            }
            cycle.observed = Some(key);
            if cycle.hold.is_none() {
                self.finish();
            }
        } else {
            self.promote(key);
        }
    }

    pub(crate) fn peek(
        &self,
        step: Step,
        projections: &[Projection],
        current: Option<&PaneKey>,
    ) -> Option<(usize, PaneKey)> {
        if self.pending() {
            return None;
        }
        let count = self.order.len();
        let cursor = self
            .cycle
            .as_ref()
            .and_then(|cycle| self.order.iter().position(|key| *key == cycle.cursor));
        (1..=count)
            .map(|offset| match (step, cursor) {
                (Step::Back, Some(cursor)) => (cursor + offset) % count,
                (Step::Back, None) => offset - 1,
                (Step::Forward, cursor) => (cursor.unwrap_or(0) + count - offset) % count,
            })
            .find_map(|index| {
                let key = self.order.get(index)?;
                (current != Some(key) && self.reachable(projections, key))
                    .then(|| (index, key.clone()))
            })
    }

    pub(crate) fn begin(&mut self, index: usize, modifiers: Modifiers) {
        if self.pending() {
            return;
        }
        let Some(key) = self.order.get(index).cloned() else {
            return;
        };
        if let Some(cycle) = &mut self.cycle {
            cycle.cursor = key;
            cycle.pending = true;
            return;
        }
        let hold = Modifiers {
            shift: false,
            ..modifiers
        };
        self.cycle = Some(Cycle {
            cursor: key,
            hold: hold.modified().then_some(hold),
            pending: true,
            confirmed: None,
            observed: None,
        });
    }

    pub(crate) fn release(&mut self, modifiers: Modifiers) {
        if self.cycle.as_ref().is_some_and(|cycle| {
            cycle
                .hold
                .is_some_and(|hold| !hold.is_subset_of(&modifiers))
        }) {
            self.finish();
        }
    }

    pub(crate) fn finish(&mut self) {
        let Some(cycle) = &mut self.cycle else {
            return;
        };
        cycle.hold = None;
        if cycle.pending {
            return;
        }
        let Some(cycle) = self.cycle.take() else {
            return;
        };
        if let Some(key) = cycle.confirmed {
            self.promote(key);
        }
        if let Some(key) = cycle.observed {
            self.promote(key);
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.cycle.as_ref().is_some_and(|cycle| cycle.pending)
    }

    pub(super) fn selected(&self) -> Option<&PaneKey> {
        self.cycle.as_ref().map(|cycle| &cycle.cursor)
    }

    pub(super) fn cancel(&mut self) {
        self.navigation = None;
        if let Some(cycle) = &mut self.cycle {
            cycle.pending = false;
        }
        self.finish();
    }

    fn forget(&mut self, endpoint: &str) {
        self.boots.remove(endpoint);
        let on = |key: &PaneKey| matches!(key, PaneKey::Herdr { endpoint: host, .. } if host == endpoint);
        if self.cycle.as_ref().is_some_and(|cycle| {
            self.order.front().is_some_and(on)
                || on(&cycle.cursor)
                || cycle.confirmed.as_ref().is_some_and(on)
                || cycle.observed.as_ref().is_some_and(on)
        }) {
            self.cancel();
        }
        self.order.retain(|key| !on(key));
    }

    fn promote(&mut self, key: PaneKey) {
        if self.order.front() == Some(&key) {
            return;
        }
        self.order.retain(|entry| *entry != key);
        self.order.push_front(key);
        self.order.truncate(Self::LIMIT);
    }

    fn reachable(&self, projections: &[Projection], key: &PaneKey) -> bool {
        match key {
            PaneKey::Core(pane) => self.core.get(pane).copied().unwrap_or(false),
            PaneKey::Herdr { endpoint, pane } => projections.iter().any(|p| {
                p.endpoint == endpoint
                    && p.available
                    && p.snapshot.is_some_and(|snapshot| {
                        self.boots.get(p.endpoint) == Some(&snapshot.boot_id)
                            && open(snapshot, pane)
                    })
            }),
        }
    }
}

fn open(snapshot: &ClientShellSnapshot, pane: &str) -> bool {
    snapshot.panes.iter().any(|p| p.pane_id == pane)
}

#[cfg(test)]
mod tests;
