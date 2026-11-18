//! How a bubble effect holds its Markdown at a moment: how large, how
//! opaque, and how far it shakes sideways. Like a pane effect's scene, it
//! is a function of the time since the effect began alone.

use super::Bubble;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    /// The text's size, as a multiple of its own.
    pub(crate) scale: f32,
    pub(crate) opacity: f32,
    /// How far it sits sideways, in points.
    pub(crate) shift: f32,
    /// Whether invisible ink's specks still move.
    pub(crate) shimmer: bool,
}

impl Pose {
    pub(crate) const REST: Self = Self {
        scale: 1.,
        opacity: 1.,
        shift: 0.,
        shimmer: false,
    };
}

/// The largest a slam starts; any larger and a long message leaves the pane.
const SLAM_FROM: f32 = 2.2;
/// How much larger loud text grows.
const LOUD: f32 = 1.45;

/// Eases `from` to `to` as `k` goes from `start` to `end`.
fn tween(k: f32, start: f32, end: f32, from: f32, to: f32) -> f32 {
    let k = ((k - start) / (end - start)).clamp(0., 1.);
    from + (to - from) * (1. - (1. - k).powi(2))
}

impl Bubble {
    /// The pose `t` seconds in, or at rest once the effect is over or
    /// never played.
    pub(crate) fn pose(self, t: Option<f32>) -> Pose {
        let length = self.length().as_secs_f32();
        let Some(t) = t.filter(|t| (0. ..length).contains(t)) else {
            return Pose::REST;
        };
        let k = t / length;
        match self {
            Self::Slam => {
                let scale = if k < 0.3 {
                    tween(k, 0., 0.3, SLAM_FROM, 0.92)
                } else if k < 0.42 {
                    tween(k, 0.3, 0.42, 0.92, 1.04)
                } else {
                    tween(k, 0.42, 0.6, 1.04, 1.)
                };
                // It lands with a shudder.
                let shake = if (0.3..0.7).contains(&k) {
                    (t * 70.).sin() * 4. * (0.7 - k) / 0.4
                } else {
                    0.
                };
                Pose {
                    scale,
                    opacity: (k / 0.25).min(1.),
                    shift: shake,
                    shimmer: false,
                }
            }
            Self::Loud => {
                let scale = if k < 0.7 {
                    tween(k, 0., 0.15, 1., LOUD)
                } else {
                    tween(k, 0.7, 1., LOUD, 1.)
                };
                let shake = if (0.15..0.5).contains(&k) {
                    (t * 45.).sin() * 2.5
                } else {
                    0.
                };
                Pose {
                    scale,
                    opacity: 1.,
                    shift: shake,
                    shimmer: false,
                }
            }
            Self::Gentle => Pose {
                scale: if k < 0.6 {
                    tween(k, 0., 0.6, 0.55, 0.6)
                } else {
                    tween(k, 0.6, 1., 0.6, 1.)
                },
                opacity: if k < 0.6 {
                    tween(k, 0., 0.6, 0.2, 0.8)
                } else {
                    tween(k, 0.6, 1., 0.8, 1.)
                },
                shift: 0.,
                shimmer: false,
            },
            Self::Ink => Pose {
                shimmer: true,
                ..Pose::REST
            },
        }
    }
}
