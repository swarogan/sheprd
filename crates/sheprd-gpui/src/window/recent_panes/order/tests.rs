#![allow(clippy::unwrap_used)]
use super::*;

/// A snapshot of `boot` with `panes` open and `focused` focused.
fn at(boot: &str, panes: &[&str], focused: Option<&str>) -> ClientShellSnapshot {
    let mut snapshot: ClientShellSnapshot = serde_json::from_str(include_str!(
        "../../../../../sheprd-protocol/tests/fixtures/endpoint-snapshot-v1.json"
    ))
    .unwrap();
    let pane = snapshot.panes[0].clone();
    snapshot.panes = panes
        .iter()
        .map(|id| {
            let mut pane = pane.clone();
            pane.pane_id = (*id).to_owned();
            pane
        })
        .collect();
    snapshot.boot_id = boot.into();
    snapshot.focused_pane_id = focused.map(str::to_owned);
    snapshot
}

const LOCAL: &str = "local";
const REMOTE: &str = "remote";
const PANES: &[&str] = &["a", "b", "c", "d"];
const HOSTS: &[&str] = &["x", "y"];

fn focusing(focused: &str) -> ClientShellSnapshot {
    at("boot", PANES, Some(focused))
}

fn remote(focused: &str) -> ClientShellSnapshot {
    at("remote-boot", HOSTS, Some(focused))
}

fn key(endpoint: &str, pane: &str) -> PaneKey {
    PaneKey::Herdr {
        endpoint: endpoint.into(),
        pane: pane.into(),
    }
}

fn projections<'a>(
    endpoints: &[(&'a str, Option<&'a ClientShellSnapshot>)],
) -> Vec<Projection<'a>> {
    endpoints
        .iter()
        .map(|&(endpoint, snapshot)| Projection {
            endpoint,
            snapshot,
            available: true,
        })
        .collect()
}

/// The window observes `endpoints` while `selected` is on screen.
fn observe(
    recent: &mut RecentPanes,
    endpoints: &[(&str, Option<&ClientShellSnapshot>)],
    selected: &str,
) {
    let focused = endpoints
        .iter()
        .find(|(endpoint, _)| *endpoint == selected)
        .and_then(|(_, snapshot)| snapshot.as_ref()?.focused_pane_id.as_deref())
        .map(|pane| key(selected, pane));
    recent.observe(&projections(endpoints), focused);
}

/// Local is the only endpoint and shows `snapshot`.
fn observe_local(recent: &mut RecentPanes, snapshot: &ClientShellSnapshot) {
    observe(recent, &[(LOCAL, Some(snapshot))], LOCAL);
}

fn peek_local(
    recent: &RecentPanes,
    step: Step,
    snapshot: &ClientShellSnapshot,
) -> Option<(usize, String)> {
    let current = snapshot.focused_pane_id.as_deref().map(|p| key(LOCAL, p));
    recent
        .peek(
            step,
            &projections(&[(LOCAL, Some(snapshot))]),
            current.as_ref(),
        )
        .map(|(index, key)| match key {
            PaneKey::Herdr { pane, .. } => (index, pane),
            PaneKey::Core(_) => panic!("Herdr fixture"),
        })
}

/// Focuses each of `panes` on Local in turn, as clicks would.
fn visited(panes: &[&str]) -> RecentPanes {
    let mut recent = RecentPanes::default();
    for pane in panes {
        observe_local(&mut recent, &focusing(pane));
    }
    recent
}

/// The order, Local's panes by name and other endpoints' as `endpoint:pane`.
fn order(recent: &RecentPanes) -> Vec<String> {
    recent
        .order
        .iter()
        .map(|key| match key {
            PaneKey::Herdr { endpoint, pane } if endpoint == LOCAL => pane.clone(),
            PaneKey::Herdr { endpoint, pane } => format!("{endpoint}:{pane}"),
            PaneKey::Core(pane) => format!("core:{pane:?}"),
        })
        .collect()
}

/// One press from `focused` with `held` down, as the window runs it once the
/// request is queued, followed by the daemon's report that the focus landed.
fn press(recent: &mut RecentPanes, step: Step, focused: &str, held: Modifiers) -> Option<String> {
    let (index, pane) = peek_local(recent, step, &focusing(focused))?;
    recent.begin(index, held);
    observe_local(recent, &focusing(&pane));
    Some(pane)
}

/// Ctrl-Tab presses from `focused` with Ctrl held throughout.
fn walk(recent: &mut RecentPanes, mut focused: String, presses: usize) -> Vec<String> {
    (0..presses)
        .map(|_| {
            focused = press(recent, Step::Back, &focused, Modifiers::control()).unwrap();
            focused.clone()
        })
        .collect()
}

/// Local focused A then B, then Remote, selected, focused X.
fn across_endpoints() -> RecentPanes {
    let mut recent = visited(&["a", "b"]);
    observe(
        &mut recent,
        &[(LOCAL, Some(&focusing("b"))), (REMOTE, Some(&remote("x")))],
        REMOTE,
    );
    recent
}

#[test]
fn focus_orders_panes_most_recent_first() {
    assert_eq!(order(&visited(&["a", "b", "c"])), ["c", "b", "a"]);
}

#[test]
fn a_repeated_snapshot_does_not_reorder() {
    let mut recent = visited(&["a", "b"]);
    observe_local(&mut recent, &focusing("b"));
    observe(&mut recent, &[(LOCAL, None)], LOCAL);
    observe_local(&mut recent, &focusing("b"));

    assert_eq!(order(&recent), ["b", "a"]);
}

#[test]
fn a_tap_toggles_between_the_two_most_recent_panes() {
    let mut recent = visited(&["a", "b", "c"]);
    let mut focused = "c".to_owned();
    let mut landed = Vec::new();
    for _ in 0..3 {
        focused = press(&mut recent, Step::Back, &focused, Modifiers::control()).unwrap();
        recent.release(Modifiers::none());
        landed.push(focused.clone());
    }

    assert_eq!(landed, ["b", "c", "b"]);
    assert_eq!(order(&recent), ["b", "c", "a"]);
}

#[test]
fn holding_walks_back_and_the_release_commits_where_it_stopped() {
    let mut recent = visited(&["a", "b", "c", "d"]);

    assert_eq!(walk(&mut recent, "d".into(), 3), ["c", "b", "a"]);
    recent.release(Modifiers::none());

    assert_eq!(order(&recent), ["a", "d", "c", "b"]);
}

#[test]
fn the_reverse_first_press_reaches_the_oldest_pane() {
    let mut recent = visited(&["a", "b", "c", "d"]);

    let pane = press(&mut recent, Step::Forward, "d", Modifiers::control_shift());

    assert_eq!(pane.as_deref(), Some("a"));
}

#[test]
fn stepping_wraps_around() {
    let mut recent = visited(&["a", "b", "c"]);

    assert_eq!(walk(&mut recent, "c".into(), 4), ["b", "a", "c", "b"]);
}

#[test]
fn closed_and_focused_panes_are_stepped_over() {
    let recent = visited(&["a", "b", "c", "d"]);
    let without_c = at("boot", &["a", "b", "d"], Some("d"));
    // A click on C that the order has not observed yet goes back to D.
    let on_c = focusing("c");

    assert_eq!(
        peek_local(&recent, Step::Back, &without_c),
        Some((2, "b".into()))
    );
    assert_eq!(
        peek_local(&recent, Step::Back, &on_c),
        Some((0, "d".into()))
    );
}

#[test]
fn nothing_to_switch_to_without_another_open_pane() {
    let recent = visited(&["a", "b"]);

    assert_eq!(
        peek_local(&recent, Step::Back, &at("boot", &["b"], Some("b"))),
        None
    );
    assert_eq!(
        peek_local(&recent, Step::Back, &at("other", PANES, Some("b"))),
        None
    );
}

#[test]
fn closing_a_pane_drops_it_from_the_order() {
    let mut recent = visited(&["a", "b", "c"]);
    observe_local(&mut recent, &at("boot", &["a", "c"], Some("c")));

    assert_eq!(order(&recent), ["c", "a"]);
}

#[test]
fn a_new_boot_resets_the_order_and_ends_the_cycle() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());

    observe_local(&mut recent, &at("other", &["x", "y"], Some("x")));

    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["x"]);
}

#[test]
fn the_order_is_frozen_while_a_cycle_is_open() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());
    observe_local(&mut recent, &at("boot", &["a", "b"], Some("a")));

    assert_eq!(order(&recent), ["b", "a"]);
}

#[test]
fn a_focus_landing_after_the_commit_changes_nothing() {
    let mut recent = visited(&["a", "b", "c"]);
    let (index, pane) = peek_local(&recent, Step::Back, &focusing("c")).unwrap();
    recent.begin(index, Modifiers::control());
    recent.release(Modifiers::none());
    observe_local(&mut recent, &focusing(&pane));

    assert_eq!(order(&recent), ["b", "c", "a"]);
}

#[test]
fn a_later_press_without_modifiers_keeps_the_original_hold() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());
    press(&mut recent, Step::Back, "b", Modifiers::none());

    assert!(recent.cycle.is_some());
    recent.release(Modifiers::none());
    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["a", "c", "b"]);
}

#[test]
fn a_press_without_a_hold_waits_for_confirmed_focus() {
    let mut recent = visited(&["a", "b", "c"]);
    let (index, _) = peek_local(&recent, Step::Back, &focusing("c")).unwrap();
    recent.begin(index, Modifiers::none());
    assert!(recent.pending());
    assert_eq!(order(&recent), ["c", "b", "a"]);
    observe_local(&mut recent, &focusing("b"));

    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["b", "c", "a"]);
}

#[test]
fn the_cycle_lasts_while_its_modifiers_stay_down() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());

    recent.release(Modifiers::control());
    recent.release(Modifiers::control() | Modifiers::alt());

    assert!(recent.cycle.is_some());
}

#[test]
fn releasing_shift_alone_does_not_commit() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Forward, "c", Modifiers::control_shift());

    recent.release(Modifiers::control());
    assert!(recent.cycle.is_some());
    recent.release(Modifiers::none());
    assert!(recent.cycle.is_none());
}

#[test]
fn finishing_twice_changes_nothing_more() {
    let mut recent = visited(&["a", "b", "c"]);
    press(&mut recent, Step::Back, "c", Modifiers::control());

    recent.finish();
    let committed = order(&recent).join(",");
    recent.finish();

    assert_eq!(order(&recent).join(","), committed);
    assert_eq!(committed, "b,c,a");
}

#[test]
fn the_order_is_bounded() {
    let panes: Vec<String> = (0..RecentPanes::LIMIT + 10)
        .map(|n| format!("p{n}"))
        .collect();
    let open: Vec<&str> = panes.iter().map(String::as_str).collect();
    let mut recent = RecentPanes::default();
    for pane in &open {
        observe_local(&mut recent, &at("boot", &open, Some(pane)));
    }

    assert_eq!(recent.order.len(), RecentPanes::LIMIT);
    assert_eq!(order(&recent)[0], *panes.last().unwrap());
}

#[test]
fn a_click_during_the_hold_wins_after_the_commit() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    press(&mut recent, Step::Back, "d", Modifiers::control());
    observe_local(&mut recent, &focusing("a"));

    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["a", "c", "d", "b"]);
    observe_local(&mut recent, &focusing("a"));
    assert_eq!(order(&recent), ["a", "c", "d", "b"]);
}

#[test]
fn every_endpoints_panes_share_one_order() {
    let recent = across_endpoints();
    let (local, remote) = (focusing("b"), remote("x"));
    let endpoints = projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]);

    assert_eq!(order(&recent), ["remote:x", "b", "a"]);
    assert_eq!(
        recent.peek(Step::Back, &endpoints, Some(&key(REMOTE, "x"))),
        Some((1, key(LOCAL, "b")))
    );
}

#[test]
fn only_the_selected_endpoints_focus_is_recorded() {
    let mut recent = across_endpoints();
    // Another client moved Local's focus while Remote is on screen.
    observe(
        &mut recent,
        &[(LOCAL, Some(&focusing("c"))), (REMOTE, Some(&remote("x")))],
        REMOTE,
    );

    assert_eq!(order(&recent), ["remote:x", "b", "a"]);
}

#[test]
fn an_unsettled_focus_records_nothing() {
    let mut recent = across_endpoints();
    let (local, remote) = (focusing("a"), remote("x"));

    recent.observe(
        &projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]),
        None,
    );

    assert_eq!(order(&recent), ["remote:x", "b", "a"]);
}

#[test]
fn a_restarted_endpoint_forgets_only_its_own_panes() {
    let mut recent = across_endpoints();
    let restarted = at("new-boot", HOSTS, Some("y"));

    observe(
        &mut recent,
        &[(LOCAL, Some(&focusing("b"))), (REMOTE, Some(&restarted))],
        LOCAL,
    );

    assert_eq!(order(&recent), ["b", "a"]);
}

#[test]
fn a_removed_endpoint_forgets_its_panes() {
    let mut recent = across_endpoints();

    observe_local(&mut recent, &focusing("b"));

    assert_eq!(order(&recent), ["b", "a"]);
}

#[test]
fn a_disconnected_endpoint_keeps_its_panes_out_of_reach() {
    let mut recent = across_endpoints();
    let (local, remote) = (focusing("b"), remote("x"));
    observe(&mut recent, &[(LOCAL, Some(&local)), (REMOTE, None)], LOCAL);
    let offline = projections(&[(LOCAL, Some(&local)), (REMOTE, None)]);
    let online = projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]);

    assert_eq!(order(&recent), ["b", "remote:x", "a"]);
    assert_eq!(
        recent.peek(Step::Back, &offline, Some(&key(LOCAL, "b"))),
        Some((2, key(LOCAL, "a")))
    );
    assert_eq!(
        recent.peek(Step::Back, &online, Some(&key(LOCAL, "b"))),
        Some((1, key(REMOTE, "x")))
    );
}

#[test]
fn a_restart_during_a_cycle_keeps_only_confirmed_focus() {
    let mut recent = across_endpoints();
    let (local, remote) = (focusing("b"), remote("x"));
    let endpoints = projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&remote))]);
    let (index, _) = recent
        .peek(Step::Back, &endpoints, Some(&key(REMOTE, "x")))
        .unwrap();
    recent.begin(index, Modifiers::control());
    observe(
        &mut recent,
        &[(LOCAL, Some(&local)), (REMOTE, Some(&remote))],
        LOCAL,
    );
    let (index, target) = recent
        .peek(Step::Back, &endpoints, Some(&key(LOCAL, "b")))
        .unwrap();
    recent.begin(index, Modifiers::control());

    // Remote restarts before the focus on A lands.
    let restarted = at("new-boot", HOSTS, Some("x"));
    recent.observe(
        &projections(&[(LOCAL, Some(&local)), (REMOTE, Some(&restarted))]),
        None,
    );

    assert_eq!(target, key(LOCAL, "a"));
    assert!(recent.cycle.is_none());
    assert_eq!(order(&recent), ["b", "a"]);
}

mod completion;
mod model_check;
