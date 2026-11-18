//! Zap: the message's letters line up across the top of the pane and sway
//! as a formation, while three pixel ships slide along the bottom and shoot
//! them down one by one; each letter hit bursts into pixels. Each row of
//! the formation takes its own bright color.

use super::{BRIGHTS, Hue, Mark, Rect, Words, between, envelope, unit};
use std::f32::consts::TAU;

/// Letters in the formation; a longer message lends only its first ones.
pub(crate) const MAX_TARGETS: usize = 24;
/// Letters in a row of the formation, at most: a narrow pane holds fewer.
const ROW: usize = 12;
/// How far apart the letters stand at most, and their rows.
const COLUMN: f32 = 30.;
/// How far the formation sways to either side at most.
const SWAY: f32 = 40.;
const LINE: f32 = 38.;
/// The letters' size.
const LETTER: f32 = 22.;
/// How long a shot takes to reach its letter.
const FLIGHT: f32 = 0.5;
/// How long a letter's burst lasts.
const BURST: f32 = 0.7;
/// When the first and the last letters are hit.
const FIRST_HIT: f32 = 0.8;
const LAST_HIT: f32 = 3.6;
const SHIPS: usize = 3;
/// A ship, drawn in pixels: a cannon on a wide hull.
const SHIP: [&str; 4] = ["..#..", "..#..", ".###.", "#####"];
/// The side of one of a ship's pixels, and of a burst's.
const PIXEL: f32 = 4.;

/// How far the formation sinks each second.
const DRIFT: f32 = 6.;

/// How the formation fits a pane: how many letters it holds, how many in a
/// row, how far apart, and how far it sways, so every letter stays inside
/// however narrow or short the pane is.
#[derive(Clone, Copy, Debug)]
struct Formation {
    count: usize,
    per_row: usize,
    column: f32,
    sway: f32,
}

impl Formation {
    fn fit(count: usize, w: f32, h: f32) -> Self {
        let sway = (w * 0.06).min(SWAY);
        // A letter's half on either side stays in the pane too.
        let room = (w - 2. * sway - LETTER * 2.).max(0.);
        let per_row = ((room / COLUMN) as usize + 1)
            .clamp(1, ROW)
            .min(count.max(1));
        let column = match per_row {
            1 => 0.,
            _ => (room / (per_row - 1) as f32).min(COLUMN),
        };
        // Rows go down to the ships, allowing for how far the formation
        // sinks before its last letter is hit; a short pane holds fewer.
        let ships = 12. + SHIP.len() as f32 * PIXEL;
        let room = h - Self::top(h) - DRIFT * LAST_HIT - ships - LETTER;
        let rows = (room.max(0.) / LINE) as usize + 1;
        Self {
            count: count.min(per_row * rows),
            per_row,
            column,
            sway,
        }
    }

    /// Where the first row's letters start.
    fn top(h: f32) -> f32 {
        h * 0.12
    }

    /// The row letter `index` stands in.
    fn row(self, index: usize) -> usize {
        index / self.per_row
    }

    /// Where letter `index` stands at `t`: its center.
    fn at(self, index: usize, t: f32, w: f32, h: f32) -> (f32, f32) {
        let column = index % self.per_row;
        let left = w / 2. - (self.per_row - 1) as f32 * self.column / 2.;
        (
            left + column as f32 * self.column + (t * 1.6).sin() * self.sway,
            Self::top(h) + self.row(index) as f32 * LINE + DRIFT * t + LETTER / 2.,
        )
    }
}

/// Where ship `index`'s cannon is at `t`.
fn ship_at(index: usize, t: f32, w: f32, h: f32) -> (f32, f32) {
    let home = w * (index as f32 + 1.) / (SHIPS as f32 + 1.);
    (
        home + (t * 2. + index as f32 * 2.1).sin() * 60.,
        h - 12. - SHIP.len() as f32 * PIXEL,
    )
}

/// When letter `index` is hit: each at its own moment, all before the end.
fn hit_time(index: usize) -> f32 {
    between(index as u32, 41, FIRST_HIT, LAST_HIT)
}

pub(super) fn zap(t: f32, length: f32, w: f32, h: f32, letters: usize, marks: &mut Vec<Mark>) {
    let fade = envelope(t, length, 0.2, 0.4);
    // The pane dims, so the formation reads over the conversation.
    marks.push(Mark::Veil {
        alpha: 0.6 * fade,
        hole: None,
    });
    let formation = Formation::fit(letters.min(MAX_TARGETS), w, h);
    for index in 0..formation.count {
        let hit = hit_time(index);
        let shooter = index % SHIPS;
        if t < hit {
            let (x, y) = formation.at(index, t, w, h);
            marks.push(Mark::Words {
                x,
                y: y - LETTER / 2.,
                size: LETTER,
                alpha: fade,
                words: Words::Letter {
                    index,
                    row: formation.row(index),
                },
            });
        }
        let fired = hit - FLIGHT;
        if (fired..hit).contains(&t) {
            // From where the ship was when it fired to where the letter is
            // when it is hit.
            let k = (t - fired) / FLIGHT;
            let (sx, sy) = ship_at(shooter, fired, w, h);
            let (lx, ly) = formation.at(index, hit, w, h);
            let (x, y) = (sx + (lx - sx) * k, sy + (ly - sy) * k);
            marks.push(Mark::Box {
                rect: Rect {
                    x: x - 1.5,
                    y: y - 5.,
                    w: 3.,
                    h: 10.,
                },
                round: 1.,
                hue: Hue::White,
                alpha: fade,
            });
        }
        let since = t - hit;
        if (0. ..BURST).contains(&since) {
            let (x, y) = formation.at(index, hit, w, h);
            let flash = 1. - since / 0.15;
            if flash > 0. {
                marks.push(Mark::Box {
                    rect: Rect {
                        x: x - 9.,
                        y: y - 9.,
                        w: 18.,
                        h: 18.,
                    },
                    round: 2.,
                    hue: Hue::White,
                    alpha: flash * fade,
                });
            }
            for piece in 0..12 {
                let id = (index * 12 + piece) as u32;
                let angle = unit(id, 43) * TAU;
                let speed = between(id, 44, 60., 170.);
                marks.push(Mark::Box {
                    rect: Rect {
                        x: x + angle.cos() * speed * since - PIXEL / 2.,
                        y: y + angle.sin() * speed * since + 60. * since * since - PIXEL / 2.,
                        w: PIXEL,
                        h: PIXEL,
                    },
                    round: 0.,
                    hue: Hue::Bright(index % BRIGHTS),
                    alpha: (1. - since / BURST) * fade,
                });
            }
        }
    }
    for ship in 0..SHIPS {
        let (x, top) = ship_at(ship, t, w, h);
        let half = SHIP[0].len() as f32 * PIXEL / 2.;
        for (row, line) in SHIP.iter().enumerate() {
            for (column, _) in line.char_indices().filter(|(_, pixel)| *pixel == '#') {
                marks.push(Mark::Box {
                    rect: Rect {
                        x: x - half + column as f32 * PIXEL,
                        y: top + row as f32 * PIXEL,
                        w: PIXEL,
                        h: PIXEL,
                    },
                    round: 0.,
                    hue: Hue::Bright(1),
                    alpha: fade,
                });
            }
        }
    }
}
