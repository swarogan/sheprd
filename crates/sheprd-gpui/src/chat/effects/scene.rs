//! What a pane effect draws at a moment: boxes, beams, words, and a veil,
//! laid out in the pane. Each frame is a function of the time since the
//! effect began alone, as the window's other motion is, so a frame never
//! depends on the one before and a test can look at any moment. Particles
//! scatter by a fixed hash, so an effect plays the same way each time.

use super::Screen;

mod zap;

use std::f32::consts::{PI, TAU};
#[cfg(test)]
pub(crate) use zap::MAX_TARGETS;

/// A box in the pane, from its top left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

impl Rect {
    fn center(self) -> (f32, f32) {
        (self.x + self.w / 2., self.y + self.h / 2.)
    }
}

/// A color, by meaning: the renderer picks it from the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Hue {
    /// One of the theme's bright colors, by its order there.
    Bright(usize),
    /// One of the golds of a celebration.
    Gold(usize),
    White,
}

/// Words an effect draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Words {
    /// The message's own words.
    Message,
    Heart,
    SmallHeart,
    /// One letter of the message's: the `index`th that is not a space,
    /// standing in a formation's `row`.
    Letter {
        index: usize,
        row: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Mark {
    /// A filled box, rounded by `round`: a confetto, a spark, or a balloon.
    Box {
        rect: Rect,
        round: f32,
        hue: Hue,
        alpha: f32,
    },
    /// A straight line, such as a laser or a balloon's string.
    Beam {
        from: (f32, f32),
        to: (f32, f32),
        width: f32,
        hue: Hue,
        alpha: f32,
    },
    /// Words centered on `x`, with their top at `y`.
    Words {
        x: f32,
        y: f32,
        size: f32,
        alpha: f32,
        words: Words,
    },
    /// The pane dimmed, but for the hole, if any.
    Veil { alpha: f32, hole: Option<Rect> },
}

/// Bright colors an effect cycles through.
pub(crate) const BRIGHTS: usize = 6;
/// Golds a celebration's sparks take.
pub(crate) const GOLDS: usize = 4;

/// A number in `0..1` the particle `index` always draws for `salt`.
fn unit(index: u32, salt: u32) -> f32 {
    let mut x = index
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(salt.wrapping_mul(0x85EB_CA6B));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1 << 24) as f32
}

fn between(index: u32, salt: u32, low: f32, high: f32) -> f32 {
    low + unit(index, salt) * (high - low)
}

/// How much an effect shows `t` seconds in: fading in over `rise` and out
/// over the last `fall` of its `length`.
fn envelope(t: f32, length: f32, rise: f32, fall: f32) -> f32 {
    (t / rise).min((length - t) / fall).clamp(0., 1.)
}

/// What `effect` draws `t` seconds in, over a pane of `pane`'s size; the
/// message that asked for it sits at `target`, when it shows, and has
/// `letters` letters that are not spaces.
pub(crate) fn scene(
    effect: Screen,
    t: f32,
    pane: Rect,
    target: Option<Rect>,
    letters: usize,
    marks: &mut Vec<Mark>,
) {
    let length = effect.length().as_secs_f32();
    if !(0. ..length).contains(&t) || pane.w <= 0. || pane.h <= 0. {
        return;
    }
    let (w, h) = (pane.w, pane.h);
    match effect {
        Screen::Balloons => balloons(t, w, h, marks),
        Screen::Celebration => celebration(t, length, w, marks),
        Screen::Confetti => confetti(t, length, w, h, marks),
        Screen::Echo => echo(t, w, h, marks),
        Screen::Fireworks => fireworks(t, length, w, h, marks),
        Screen::Lasers => lasers(t, length, w, h, marks),
        Screen::Love => love(t, length, w, h, marks),
        Screen::Spotlight => spotlight(t, length, w, h, target, marks),
        Screen::Zap => zap::zap(t, length, w, h, letters, marks),
    }
}

fn confetti(t: f32, length: f32, w: f32, h: f32, marks: &mut Vec<Mark>) {
    let alpha = envelope(t, length, 0.05, 0.6);
    for i in 0..150 {
        let phase = between(i, 1, 0., TAU);
        let speed = h * between(i, 2, 0.45, 0.8);
        let x = between(i, 3, 0., w) + between(i, 4, -40., 40.) * t + (t * 3. + phase).sin() * 24.;
        let y = between(i, 5, -0.6, 0.) * h - 12. + speed * t;
        // A confetto flips as it falls: its height follows the flip.
        let (pw, ph) = (between(i, 6, 5., 9.), between(i, 7, 8., 14.));
        let flip = (t * 6. + phase).cos().abs().max(0.12);
        marks.push(Mark::Box {
            rect: Rect {
                x: x - pw / 2.,
                y: y - ph * flip / 2.,
                w: pw,
                h: ph * flip,
            },
            round: 1.,
            hue: Hue::Bright(i as usize % BRIGHTS),
            alpha,
        });
    }
}

fn balloons(t: f32, w: f32, h: f32, marks: &mut Vec<Mark>) {
    for i in 0..14 {
        let size = between(i, 1, 22., 34.);
        let speed = h * between(i, 2, 0.42, 0.6);
        let x =
            between(i, 3, 24., (w - 24.).max(24.)) + (t * 1.5 + between(i, 4, 0., TAU)).sin() * 12.;
        let y = h + size + between(i, 5, 0., 0.5) * h - speed * t;
        let hue = Hue::Bright(i as usize % BRIGHTS);
        marks.push(Mark::Beam {
            from: (x, y + size),
            to: (x + 4., y + size * 3.),
            width: 1.,
            hue: Hue::White,
            alpha: 0.5,
        });
        marks.push(Mark::Box {
            rect: Rect {
                x: x - size * 0.82,
                y: y - size,
                w: size * 1.64,
                h: size * 2.,
            },
            round: size * 0.82,
            hue,
            alpha: 0.95,
        });
        // The shine on its upper left.
        marks.push(Mark::Box {
            rect: Rect {
                x: x - size * 0.48,
                y: y - size * 0.7,
                w: size * 0.36,
                h: size * 0.6,
            },
            round: size * 0.18,
            hue: Hue::White,
            alpha: 0.35,
        });
    }
}

fn fireworks(t: f32, length: f32, w: f32, h: f32, marks: &mut Vec<Mark>) {
    marks.push(Mark::Veil {
        alpha: 0.55 * envelope(t, length, 0.4, 0.5),
        hole: None,
    });
    const ROCKET: f32 = 1.1;
    for rocket in 0..9 {
        let launch = rocket as f32 * 0.33;
        let x = w * between(rocket, 1, 0.2, 0.8);
        let top = h * between(rocket, 2, 0.15, 0.45);
        let flight = ROCKET * (h - top) / h;
        let since = t - launch;
        if since < 0. {
            break;
        }
        if since < flight {
            let y = h - (h - top) * since / flight;
            marks.push(Mark::Box {
                rect: Rect {
                    x: x - 1.,
                    y,
                    w: 2.,
                    h: 9.,
                },
                round: 1.,
                hue: Hue::White,
                alpha: 0.9,
            });
            continue;
        }
        let burst = since - flight;
        let life = 1. - burst * 0.8;
        if life <= 0. {
            continue;
        }
        for spark in 0..64 {
            let id = rocket * 64 + spark;
            let angle = between(id, 3, 0., TAU);
            let speed = between(id, 4, 40., 170.);
            let sx = x + angle.cos() * speed * burst;
            let sy = top + angle.sin() * speed * burst + 45. * burst * burst;
            marks.push(Mark::Box {
                rect: Rect {
                    x: sx - 2.,
                    y: sy - 2.,
                    w: 4.,
                    h: 4.,
                },
                round: 2.,
                hue: Hue::Bright(rocket as usize % BRIGHTS),
                alpha: life,
            });
        }
    }
}

fn lasers(t: f32, length: f32, w: f32, h: f32, marks: &mut Vec<Mark>) {
    let fade = envelope(t, length, 0.3, 0.3);
    marks.push(Mark::Veil {
        alpha: 0.75 * fade,
        hole: None,
    });
    let center = (w / 2., h * 0.45);
    let reach = w.max(h) * 1.5;
    for i in 0..10 {
        let turn = if i % 2 == 0 { -1.1 } else { 1.3 };
        let angle = t * turn + i as f32 * 0.63;
        let to = (
            center.0 + angle.cos() * reach,
            center.1 + angle.sin() * reach,
        );
        let hue = Hue::Bright(i as usize % BRIGHTS);
        // A wide faint beam under a narrow bright one reads as a glow.
        for (width, alpha) in [(9., 0.2), (3., 0.85)] {
            marks.push(Mark::Beam {
                from: center,
                to,
                width,
                hue,
                alpha: alpha * fade,
            });
        }
    }
    marks.push(Mark::Words {
        x: center.0,
        y: center.1 - 14.,
        size: 22.,
        alpha: fade,
        words: Words::Message,
    });
}

fn celebration(t: f32, length: f32, w: f32, marks: &mut Vec<Mark>) {
    const SPARKS: u32 = 240;
    const POURING: f32 = 2.4;
    for i in 0..SPARKS {
        let born = i as f32 * POURING / SPARKS as f32;
        let age = t - born;
        if age < 0. {
            break;
        }
        let life = (1. - age * 0.55).min((length - t) / 0.4);
        if life <= 0. {
            continue;
        }
        // Down and to the left, out of the top right corner.
        let angle = between(i, 1, PI * 0.5, PI * 1.05);
        let speed = between(i, 2, 120., 380.);
        let x = w + angle.cos() * speed * age;
        let y = angle.sin().abs() * speed * age + 20. * age * age;
        let size = between(i, 3, 2., 6.4);
        marks.push(Mark::Box {
            rect: Rect {
                x: x - size / 2.,
                y: y - size / 2.,
                w: size,
                h: size,
            },
            round: size / 2.,
            hue: Hue::Gold(i as usize % GOLDS),
            alpha: life,
        });
    }
}

fn echo(t: f32, w: f32, h: f32, marks: &mut Vec<Mark>) {
    for i in 0..28 {
        let delay = i as f32 * 0.06;
        let span = between(i, 1, 1.8, 3.);
        let k = (t - delay) / span;
        if !(0. ..1.).contains(&k) {
            continue;
        }
        let eased = 1. - (1. - k).powi(3);
        let alpha = 0.9 * (k / 0.15).min((1. - k) / 0.2).clamp(0., 1.);
        marks.push(Mark::Words {
            x: between(i, 2, 0.05, 0.95) * w,
            y: h + 20. - (h + 100.) * eased,
            size: between(i, 3, 12., 30.),
            alpha,
            words: Words::Message,
        });
    }
}

fn spotlight(t: f32, length: f32, w: f32, h: f32, target: Option<Rect>, marks: &mut Vec<Mark>) {
    // A message spans the column, but its words start at its left: light
    // that part, no wider than half the pane.
    let lit = target.map(|target| Rect {
        w: target.w.min(w * 0.5),
        ..target
    });
    let (tx, ty) = lit.map_or((w / 2., h / 2.), Rect::center);
    let (tw, th) = lit.map_or((160., 60.), |lit| (lit.w + 60., lit.h + 40.));
    let k = (t / 1.2).min(1.);
    let eased = 1. - (1. - k).powi(3);
    // It sweeps in from the upper left as a small round light, wavering
    // until it settles on the message.
    let x = w * 0.15 + (tx - w * 0.15) * eased + (t * 4.).sin() * 30. * (1. - eased);
    let y = h * 0.2 + (ty - h * 0.2) * eased;
    let (rw, rh) = (120. + (tw - 120.) * eased, 90. + (th - 90.) * eased);
    marks.push(Mark::Veil {
        alpha: 0.78 * envelope(t, length, 0.25, 0.5),
        hole: Some(Rect {
            x: x - rw / 2.,
            y: y - rh / 2.,
            w: rw,
            h: rh,
        }),
    });
}

fn love(t: f32, length: f32, w: f32, h: f32, marks: &mut Vec<Mark>) {
    let fade = envelope(t, length, 0.05, 0.5);
    for i in 0..26 {
        let speed = between(i, 1, 50., 110.);
        marks.push(Mark::Words {
            x: between(i, 2, 0., w) + (t * 2. + between(i, 3, 0., TAU)).sin() * 10.,
            y: h + between(i, 4, 0., 200.) - speed * t,
            size: between(i, 5, 10., 20.),
            alpha: fade,
            words: Words::SmallHeart,
        });
    }
    // The heart beats twice, then rises away.
    let k = t / length;
    let beat = match k {
        k if k < 0.2 => 1.1 * k / 0.2,
        k if k < 0.3 => 1.1 - 0.15 * (k - 0.2) / 0.1,
        k if k < 0.4 => 0.95 + 0.2 * (k - 0.3) / 0.1,
        k if k < 0.55 => 1.15 - 0.15 * (k - 0.4) / 0.15,
        _ => 1.,
    };
    let rise = ((k - 0.55) / 0.45).max(0.) * h * 0.2;
    let size = 120. * beat;
    marks.push(Mark::Words {
        x: w / 2.,
        y: h * 0.45 - size / 2. - rise,
        size,
        alpha: if k < 0.85 { 1. } else { (1. - k) / 0.15 },
        words: Words::Heart,
    });
}

/// Where invisible ink's speck `index` sits for `frame`, as fractions of the
/// text's width and height, and how dark it is.
pub(crate) fn speck(index: u32, frame: u32) -> (f32, f32, f32) {
    let salt = frame.wrapping_mul(3);
    (
        unit(index, salt + 11),
        unit(index, salt + 12),
        between(index, salt + 13, 0.25, 1.),
    )
}
