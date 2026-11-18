#![allow(clippy::unwrap_used)]

use std::time::{Duration, SystemTime};

use super::*;
use crate::{
    model::{Account, Kind, SESSION, WEEK, Window},
    registry,
};

const NOW: Duration = Duration::from_secs(1_000_000);

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

/// A Claude report with one five-hour window `used` percent spent, that
/// resets in `left`.
fn session(used: f64, left: Duration) -> Report {
    Report::new(
        registry::find("claude").unwrap(),
        Account::default(),
        vec![Window::new(
            Kind::Session,
            used,
            Some(now() + left),
            Some(SESSION),
        )],
    )
}

/// A report with `windows`, each `(kind, used, resets in, length)`.
fn windows(windows: &[(Kind, f64, Duration, Duration)]) -> Report {
    Report::new(
        registry::find("claude").unwrap(),
        Account::default(),
        windows
            .iter()
            .map(|(kind, used, left, length)| {
                Window::new(kind.clone(), *used, Some(now() + *left), Some(*length))
            })
            .collect(),
    )
}

fn hours(hours: u64) -> Duration {
    Duration::from_secs(hours * 3600)
}

#[test]
fn room_reads_the_tightest_window() {
    assert_eq!(Room::of(None, now()), Room::Unknown);
    // An hour into the window, a fifth used: on pace to last.
    assert_eq!(
        Room::of(Some(&session(20., hours(4))), now()),
        Room::Ample { used: 20. }
    );
    assert!(matches!(
        Room::of(Some(&session(85., hours(4))), now()),
        Room::Tight { used, resets_in: Some(left), .. } if used == 85. && left == hours(4)
    ));
    assert_eq!(
        Room::of(Some(&session(100., hours(2))), now()),
        Room::Spent {
            resets_in: Some(hours(2))
        }
    );
}

#[test]
fn a_plan_on_pace_to_run_out_is_tight_only_past_half() {
    // Two thirds used an hour in: gone well before the reset.
    let running = Room::of(Some(&session(66., hours(4))), now());
    assert!(
        matches!(
            running,
            Room::Tight {
                runs_out: Some(_),
                ..
            }
        ),
        "{running:?}"
    );
    // A third used an hour in also runs out early, but an early burst in
    // a fresh window is not worth leaving the preferred agent for.
    assert_eq!(
        Room::of(Some(&session(33., hours(4))), now()),
        Room::Ample { used: 33. }
    );
}

#[test]
fn the_preferred_agent_stays_while_its_plan_has_room() {
    let candidates = [
        ("claude", Room::Ample { used: 60. }),
        ("codex", Room::Ample { used: 5. }),
    ];
    assert_eq!(
        pick(&candidates, Some("claude")),
        Some(Pick {
            chosen: "claude",
            instead_of: None
        })
    );
    // Nothing known is no reason to leave it either.
    let candidates = [
        ("claude", Room::Unknown),
        ("codex", Room::Ample { used: 5. }),
    ];
    assert_eq!(pick(&candidates, Some("claude")).unwrap().chosen, "claude");
}

#[test]
fn a_short_plan_gives_way_to_the_most_room() {
    let tight = Room::Tight {
        used: 90.,
        resets_in: Some(hours(2)),
        runs_out: None,
    };
    let candidates = [
        ("claude", tight),
        ("gemini", Room::Unknown),
        ("codex", Room::Ample { used: 40. }),
        ("pi", Room::Ample { used: 10. }),
    ];
    assert_eq!(
        pick(&candidates, Some("claude")),
        Some(Pick {
            chosen: "pi",
            instead_of: Some(("claude", tight)),
        })
    );
    // With no plan known to have room, one not known at all still beats it.
    let candidates = [("claude", tight), ("gemini", Room::Unknown)];
    assert_eq!(pick(&candidates, Some("claude")).unwrap().chosen, "gemini");
}

#[test]
fn a_tight_plan_is_not_traded_for_another_tight_one() {
    let tight = |used| Room::Tight {
        used,
        resets_in: None,
        runs_out: None,
    };
    let candidates = [("claude", tight(85.)), ("codex", tight(81.))];
    assert_eq!(pick(&candidates, Some("claude")).unwrap().instead_of, None);
    // A spent plan gives way even to a tight one.
    let spent = Room::Spent { resets_in: None };
    let candidates = [("claude", spent), ("codex", tight(95.))];
    assert_eq!(
        pick(&candidates, Some("claude")),
        Some(Pick {
            chosen: "codex",
            instead_of: Some(("claude", spent)),
        })
    );
}

#[test]
fn without_a_preference_room_alone_decides() {
    let candidates = [
        ("claude", Room::Ample { used: 50. }),
        ("codex", Room::Ample { used: 20. }),
    ];
    assert_eq!(
        pick(&candidates, None),
        Some(Pick {
            chosen: "codex",
            instead_of: None
        })
    );
    // An agent no longer installed is no preference.
    assert_eq!(pick(&candidates, Some("gemini")).unwrap().chosen, "codex");
    assert_eq!(pick::<&str>(&[], Some("claude")), None);
}

#[test]
fn a_running_out_window_counts_though_another_is_fuller() {
    // The session is 70% used with an hour left: on pace to last. The week
    // is 60% used a day in: gone well before it resets.
    let report = windows(&[
        (Kind::Session, 70., hours(1), SESSION),
        (Kind::Weekly, 60., hours(6 * 24), WEEK),
    ]);
    let room = Room::of(Some(&report), now());
    assert!(
        matches!(room, Room::Tight { used, runs_out: Some(_), .. } if used == 60.),
        "{room:?}"
    );
}

#[test]
fn a_window_past_its_reset_says_nothing() {
    let spent_yesterday = Report::new(
        registry::find("claude").unwrap(),
        Account::default(),
        vec![Window::new(
            Kind::Session,
            100.,
            Some(now() - hours(1)),
            Some(SESSION),
        )],
    );
    assert_eq!(Room::of(Some(&spent_yesterday), now()), Room::Unknown);
    // Another window still in force decides alone.
    let mut report = spent_yesterday;
    report.windows.push(Window::new(
        Kind::Weekly,
        20.,
        Some(now() + hours(48)),
        Some(WEEK),
    ));
    assert_eq!(Room::of(Some(&report), now()), Room::Ample { used: 20. });
}
