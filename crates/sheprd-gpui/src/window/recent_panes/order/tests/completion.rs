use super::*;

fn accept(recent: &mut RecentPanes, focused: &str) -> String {
    let (index, pane) = peek_local(recent, Step::Back, &focusing(focused)).unwrap();
    recent.begin(index, Modifiers::control());
    pane
}

#[test]
fn release_before_completion_keeps_original_history_until_observed() {
    let mut recent = visited(&["a", "b", "c"]);
    assert_eq!(accept(&mut recent, "c"), "b");
    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["c", "b", "a"]);
    assert!(peek_local(&recent, Step::Back, &focusing("c")).is_none());
    observe_local(&mut recent, &focusing("b"));
    assert_eq!(order(&recent), ["b", "c", "a"]);
    assert!(!recent.pending());
}

#[test]
fn cancelled_unobserved_request_is_not_a_visit() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    recent.release(Modifiers::none());
    recent.cancel();
    assert_eq!(order(&recent), ["d", "c", "b", "a"]);
    assert!(!recent.pending());
}

#[test]
fn cancellation_keeps_the_last_confirmed_selection_and_later_actual_focus() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    observe_local(&mut recent, &focusing("a"));
    assert_eq!(accept(&mut recent, "a"), "b");
    recent.cancel();
    assert_eq!(order(&recent), ["a", "c", "d", "b"]);
}

#[test]
fn coalesced_completion_then_independent_focus_keeps_both_visits() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    observe_local(&mut recent, &focusing("a"));
    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["a", "c", "d", "b"]);
}

#[test]
fn closing_a_pending_target_cancels_without_promoting_it() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    recent.release(Modifiers::none());
    recent.observe(
        &projections(&[(LOCAL, Some(&at("boot", &["a", "b", "d"], Some("d"))))]),
        None,
    );
    assert_eq!(order(&recent), ["d", "b", "a"]);
    assert!(!recent.pending());
}

#[test]
fn pruning_an_intermediate_entry_keeps_the_cursor_on_its_pane() {
    let mut recent = visited(&["a", "b", "c", "d"]);
    assert_eq!(accept(&mut recent, "d"), "c");
    observe_local(&mut recent, &focusing("c"));
    assert_eq!(accept(&mut recent, "c"), "b");
    observe_local(&mut recent, &at("boot", &["a", "b", "d"], Some("b")));
    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["b", "d", "a"]);
}

#[test]
fn identical_ids_on_different_endpoints_remain_distinct() {
    let mut recent = visited(&["a", "b"]);
    let local = focusing("b");
    let remote = at("remote-boot", PANES, Some("b"));
    let endpoints = [(LOCAL, Some(&local)), (REMOTE, Some(&remote))];
    observe(&mut recent, &endpoints, REMOTE);
    let projected = projections(&endpoints);
    let (index, destination) = recent
        .peek(Step::Back, &projected, Some(&key(REMOTE, "b")))
        .unwrap();
    assert_eq!(destination, key(LOCAL, "b"));
    recent.begin(index, Modifiers::control());
    recent.release(Modifiers::none());
    assert_eq!(order(&recent), ["remote:b", "b", "a"]);
    observe(&mut recent, &endpoints, LOCAL);
    assert_eq!(order(&recent), ["b", "remote:b", "a"]);
}

#[test]
fn core_panes_survive_herdr_boot_changes_and_skip_stale_links() {
    use crate::{browser::TabId, core_terminals::recent::RecentCorePane};
    use sheprd_backend::TerminalId;
    let core = RecentCorePane::Tab {
        tab: TabId::from(7),
        terminal: TerminalId::new("t1").unwrap(),
    };
    let mut recent = visited(&["a", "b"]);
    recent.sync_core([(core.clone(), true)]);
    recent.observe(
        &projections(&[(LOCAL, Some(&focusing("b")))]),
        Some(PaneKey::Core(core.clone())),
    );
    observe_local(&mut recent, &at("new", PANES, Some("a")));
    assert!(recent.order.contains(&PaneKey::Core(core.clone())));
    recent.sync_core([(core.clone(), false)]);
    assert!(peek_local(&recent, Step::Back, &at("new", PANES, Some("a"))).is_none());
    assert!(recent.order.contains(&PaneKey::Core(core)));
    recent.sync_core([]);
    observe_local(&mut recent, &at("new", PANES, Some("a")));
    assert_eq!(order(&recent), ["a"]);
}
