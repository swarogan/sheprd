use super::{
    Bubble, Pose, Screen,
    scene::{Mark, Rect, Words, scene, speck},
};

const PANE: Rect = Rect {
    x: 0.,
    y: 0.,
    w: 480.,
    h: 640.,
};

fn marks(effect: Screen, t: f32) -> Vec<Mark> {
    let mut marks = Vec::new();
    scene(effect, t, PANE, None, 20, &mut marks);
    marks
}

#[test]
fn every_pane_effect_draws_while_it_plays_and_nothing_after() {
    for effect in Screen::ALL {
        let length = effect.length().as_secs_f32();
        assert!(!marks(*effect, length * 0.4).is_empty(), "{effect:?}");
        assert!(marks(*effect, length).is_empty(), "{effect:?}");
        assert!(marks(*effect, -0.1).is_empty(), "{effect:?}");
    }
}

#[test]
fn a_frame_depends_on_the_time_alone() {
    for effect in Screen::ALL {
        assert_eq!(marks(*effect, 1.3), marks(*effect, 1.3), "{effect:?}");
    }
}

#[test]
fn every_mark_is_visible_or_faint_never_beyond_opaque() {
    for effect in Screen::ALL {
        let length = effect.length().as_secs_f32();
        for step in 0..40 {
            for mark in marks(*effect, length * step as f32 / 40.) {
                let alpha = match mark {
                    Mark::Box { alpha, .. }
                    | Mark::Beam { alpha, .. }
                    | Mark::Words { alpha, .. }
                    | Mark::Veil { alpha, .. } => alpha,
                };
                assert!((0. ..=1.).contains(&alpha), "{effect:?} {mark:?}");
            }
        }
    }
}

#[test]
fn a_spotlight_settles_on_its_message() {
    let target = Rect {
        x: 40.,
        y: 300.,
        w: 200.,
        h: 40.,
    };
    let mut marks = Vec::new();
    scene(Screen::Spotlight, 2., PANE, Some(target), 20, &mut marks);
    let [
        Mark::Veil {
            hole: Some(hole), ..
        },
    ] = marks[..]
    else {
        panic!("{marks:?}");
    };
    assert!(
        hole.x <= target.x && hole.x + hole.w >= target.x + target.w,
        "{hole:?}"
    );
    assert!(
        hole.y <= target.y && hole.y + hole.h >= target.y + target.h,
        "{hole:?}"
    );
}

#[test]
fn an_empty_pane_draws_nothing() {
    let mut marks = Vec::new();
    scene(
        Screen::Confetti,
        1.,
        Rect { w: 0., ..PANE },
        None,
        20,
        &mut marks,
    );
    assert!(marks.is_empty());
}

#[test]
fn a_bubble_rests_when_it_never_played_or_is_over() {
    for effect in Bubble::ALL {
        assert_eq!(effect.pose(None), Pose::REST);
        assert_eq!(effect.pose(Some(effect.length().as_secs_f32())), Pose::REST);
    }
}

#[test]
fn a_slam_lands_from_large_and_loud_grows_then_returns() {
    let slam = Bubble::Slam.pose(Some(0.));
    assert!(slam.scale > 2. && slam.opacity < 0.1, "{slam:?}");
    let loud = Bubble::Loud.pose(Some(0.6));
    assert!(loud.scale > 1.4, "{loud:?}");
    let gentle = Bubble::Gentle.pose(Some(0.2));
    assert!(gentle.scale < 0.7 && gentle.opacity < 0.5, "{gentle:?}");
    assert!(Bubble::Ink.pose(Some(1.)).shimmer);
}

#[test]
fn specks_stay_inside_the_text_they_hide() {
    for frame in [0, 1, 99] {
        for index in 0..500 {
            let (x, y, alpha) = speck(index, frame);
            assert!((0. ..1.).contains(&x) && (0. ..1.).contains(&y));
            assert!((0. ..=1.).contains(&alpha));
        }
    }
    assert_ne!(speck(3, 1), speck(3, 2), "they shimmer from frame to frame");
}

#[test]
fn zap_shoots_down_every_letter_before_it_ends() {
    let letters = |t: f32| {
        let mut marks = Vec::new();
        scene(Screen::Zap, t, PANE, None, 20, &mut marks);
        marks
            .iter()
            .filter(|mark| {
                matches!(
                    mark,
                    Mark::Words {
                        words: Words::Letter { .. },
                        ..
                    }
                )
            })
            .count()
    };
    assert_eq!(letters(0.1), 20, "the whole formation shows at first");
    let end = Screen::Zap.length().as_secs_f32();
    assert_eq!(letters(end - 0.6), 0, "every letter is hit by the end");
    assert!(letters(2.) < 20, "some are hit along the way");
}

#[test]
fn zap_draws_at_most_its_formation() {
    let mut marks = Vec::new();
    scene(Screen::Zap, 0.1, PANE, None, 500, &mut marks);
    let shown = marks
        .iter()
        .filter(|mark| {
            matches!(
                mark,
                Mark::Words {
                    words: Words::Letter { .. },
                    ..
                }
            )
        })
        .count();
    assert_eq!(shown, super::scene::MAX_TARGETS);
}

#[test]
fn zap_keeps_every_letter_inside_a_narrow_or_short_pane() {
    for (width, height) in [
        (120., 300.),
        (120., 80.),
        (200., 160.),
        (300., 640.),
        (480., 120.),
        (1200., 640.),
    ] {
        let pane = Rect {
            w: width,
            h: height,
            ..PANE
        };
        for step in 0..46 {
            let t = step as f32 * 0.1;
            let mut marks = Vec::new();
            scene(Screen::Zap, t, pane, None, 24, &mut marks);
            for mark in marks {
                if let Mark::Words {
                    x,
                    y,
                    size,
                    words: Words::Letter { .. },
                    ..
                } = mark
                {
                    // A letter is about half as wide as it is tall; `y` is its top.
                    assert!(
                        x - size / 2. >= 0. && x + size / 2. <= width,
                        "{width}x{height}, t {t}: letter at x {x}"
                    );
                    assert!(
                        y >= 0. && y + size <= height,
                        "{width}x{height}, t {t}: letter at y {y}"
                    );
                }
            }
        }
    }
}
