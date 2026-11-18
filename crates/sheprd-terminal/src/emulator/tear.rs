//! A synchronized update a graphics command tore open. The parser holds an
//! update's bytes until it ends, so the screen keeps the last complete
//! frame; a graphics command must instead apply what came before it, so its
//! placement lands at the right cursor. From then until the update ends the
//! screen is part old and part new, and the takes hand out what the screen
//! showed just before the tear instead.

use crate::{
    damage::{Damage, Tracker},
    graphics::Graphics,
    listener::Listener,
};
use alacritty_terminal::{Term, term::TermMode};
use sheprd_protocol::SurfaceGraphicsScene;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub(super) struct Tear {
    /// Whether the screen holds part of a pending update.
    pub(super) open: bool,
    /// What changed on screen before the tear and was not taken yet.
    damage: Option<Damage>,
    /// The images on screen before the tear, when not taken yet.
    graphics: Option<SurfaceGraphicsScene>,
    /// How many synchronized updates have ended.
    pub(super) ended: u64,
}

impl Tear {
    /// Keeps what the screen shows before a graphics command applies part
    /// of the pending update.
    pub(super) fn open(
        &mut self,
        term: &mut Term<Listener>,
        tracker: &mut Tracker,
        graphics: &mut Graphics,
    ) {
        if self.open {
            return;
        }
        self.open = true;
        // Rows kept from an earlier tear and rows changed since cannot be
        // told apart, so they go together as one full frame.
        if self.damage.is_some() {
            tracker.full = true;
        }
        let alt_screen = term.mode().contains(TermMode::ALT_SCREEN);
        let damage = tracker.take(term, alt_screen);
        if !damage.is_empty() {
            self.damage = Some(damage);
        }
        if graphics.active() {
            let found = super::markers(graphics, term);
            let scene = graphics.scene(&found);
            self.graphics = merge(self.graphics.take(), scene);
        }
    }

    /// The pending update ended, torn or not: the screen is whole again.
    pub(super) fn end(&mut self, tracker: &mut Tracker) {
        self.ended = self.ended.wrapping_add(1);
        self.open = false;
        // The tracker missed what changed before the tear: send it all.
        if self.damage.take().is_some() {
            tracker.full = true;
        }
    }

    /// The damage to send while torn: what changed before the tear.
    pub(super) fn take_damage(&mut self) -> Damage {
        self.damage.take().unwrap_or(Damage::Rows {
            rows: Vec::new(),
            cursor: None,
        })
    }

    /// The scene from before the tear, not taken yet.
    pub(super) fn take_graphics(&mut self) -> Option<SurfaceGraphicsScene> {
        self.graphics.take()
    }
}

/// One scene for `older` and `newer` taken together. `newer` left out the
/// bytes of images `older` carried, as if `older` had been sent.
pub(super) fn merge(
    older: Option<SurfaceGraphicsScene>,
    newer: Option<SurfaceGraphicsScene>,
) -> Option<SurfaceGraphicsScene> {
    let (older, mut newer) = match (older, newer) {
        (Some(older), Some(newer)) => (older, newer),
        (older, None) => return older,
        (None, newer) => return newer,
    };
    let placed: HashSet<_> = newer.placements.iter().map(|p| &p.asset).collect();
    let carried: HashSet<_> = newer.assets.iter().map(|a| &a.key).collect();
    let missing: Vec<_> = older
        .assets
        .into_iter()
        .filter(|asset| placed.contains(&asset.key) && !carried.contains(&asset.key))
        .collect();
    newer.assets.extend(missing);
    Some(newer)
}
