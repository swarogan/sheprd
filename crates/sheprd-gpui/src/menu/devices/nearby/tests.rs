#![allow(clippy::unwrap_used)]
use super::{Event, Found, NearbySetup, Page, Step};
use crate::{SheprdWindow, sidebar::layout_tests::fixture_window};
use gpui::{Context, TestAppContext, VisualTestContext};
use sheprd_remote_control::control::desktop::PairOutput;

const STUDIO: &str = "1A2B3C4D-5E6F-7081-92A3-B4C5D6E7F809";

/// The page as it opens, scanning with no helper behind it.
fn open(view: &mut SheprdWindow) {
    view.menu.nearby_setup = Some(NearbySetup {
        attempt: super::next_attempt(),
        found: Vec::new(),
        error: None,
        pairing: None,
        radio: None,
        scanner: None,
        step: Step::Looking,
        _poll: None,
    });
    view.menu.page = Some(Page::AddNearby);
}

fn setup(view: &SheprdWindow) -> &NearbySetup {
    view.menu.nearby_setup.as_ref().unwrap()
}

fn found(peripheral: &str, name: &str) -> Event {
    Event::Found {
        peripheral: peripheral.into(),
        name: name.into(),
    }
}

fn studio() -> Found {
    Found {
        peripheral: STUDIO.into(),
        name: "Studio".into(),
    }
}

fn window(cx: &mut TestAppContext) -> (gpui::Entity<SheprdWindow>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(fixture_window);
    view.update_in(cx, |view, _, _| open(view));
    (view, cx)
}

#[gpui::test]
fn computers_found_are_listed_once_and_counted_for_the_rail(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, _, cx: &mut Context<SheprdWindow>| {
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.nearby_event(found("00000000-0000-0000-0000-000000000001", "Laptop"), cx);
        assert_eq!(setup(view).found(), 2);
        // A computer that leaves Bluetooth range leaves the list.
        view.nearby_event(
            Event::PeripheralDisconnected {
                peripheral: STUDIO.into(),
            },
            cx,
        );
        assert_eq!(
            setup(view).found,
            [Found {
                peripheral: "00000000-0000-0000-0000-000000000001".into(),
                name: "Laptop".into(),
            }]
        );
    });
}

#[gpui::test]
fn picking_a_computer_connects_and_losing_it_before_the_code_stops(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, _, cx| {
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.pair_nearby(STUDIO, cx);
        assert_eq!(setup(view).step, Step::Connecting(studio()));
        // Another computer's readiness is not this one's.
        view.nearby_event(
            Event::Connected {
                peripheral: "00000000-0000-0000-0000-000000000001".into(),
            },
            cx,
        );
        assert_eq!(setup(view).step, Step::Connecting(studio()));
        view.nearby_event(
            Event::PeripheralDisconnected {
                peripheral: STUDIO.into(),
            },
            cx,
        );
        assert_eq!(setup(view).step, Step::Looking);
        assert!(setup(view).error.as_deref().unwrap().contains("Studio"));
    });
}

#[gpui::test]
fn the_code_shows_and_bluetooth_may_go_once_it_does(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, window, cx| {
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.pair_nearby(STUDIO, cx);
        view.menu.nearby_setup.as_mut().unwrap().step = Step::Exchanging(studio());
        view.nearby_output(
            PairOutput::Code {
                code: "428913".into(),
                computer: "Studio Mac".into(),
            },
            window,
            cx,
        );
        let showing = |claimed| Step::Code {
            computer: Found {
                peripheral: STUDIO.into(),
                name: "Studio Mac".into(),
            },
            code: "428913".into(),
            claimed,
        };
        assert_eq!(setup(view).step, showing(false));
        view.nearby_output(PairOutput::Claimed, window, cx);
        assert_eq!(setup(view).step, showing(true));
        // The claim goes through the relays: Bluetooth leaving is fine now.
        view.nearby_event(
            Event::PeripheralDisconnected {
                peripheral: STUDIO.into(),
            },
            cx,
        );
        assert_eq!(setup(view).step, showing(true));
        assert_eq!(setup(view).error, None);
    });
}

#[gpui::test]
fn a_failed_pairing_says_why_and_lists_again(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, window, cx| {
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.pair_nearby(STUDIO, cx);
        view.nearby_output(
            PairOutput::Failed {
                message: "the other computer's user did not allow it".into(),
            },
            window,
            cx,
        );
        assert_eq!(setup(view).step, Step::Looking);
        assert!(
            setup(view)
                .error
                .as_deref()
                .unwrap()
                .contains("did not allow it")
        );
    });
}

#[gpui::test]
fn escape_cancels_a_pairing_before_closing_the_page(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, window, cx| {
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.pair_nearby(STUDIO, cx);
        let escape = gpui::KeyDownEvent {
            keystroke: gpui::Keystroke {
                modifiers: gpui::Modifiers::default(),
                key: "escape".into(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };
        view.add_nearby_key(&escape, window, cx);
        assert_eq!(setup(view).step, Step::Looking);
        assert_eq!(view.menu.page, Some(Page::AddNearby));
    });
}

#[gpui::test]
fn each_pick_and_cancel_starts_a_new_attempt(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |view, _, cx| {
        view.nearby_event(found(STUDIO, "Studio"), cx);
        view.pair_nearby(STUDIO, cx);
        let picked = setup(view).attempt;
        view.stop_nearby_pairing(cx);
        let cancelled = setup(view).attempt;
        view.pair_nearby(STUDIO, cx);
        // A save or a child started for an earlier attempt finds another
        // number and leaves this one alone.
        assert!(picked < cancelled && cancelled < setup(view).attempt);
        assert_eq!(setup(view).step, Step::Connecting(studio()));
    });
}

#[test]
fn a_reopened_page_never_reuses_an_attempt() {
    // A page closed with a save under way and opened again draws a new
    // number, so the old save's answer finds no match.
    let closed = super::next_attempt();
    let reopened = super::next_attempt();
    assert_ne!(closed, reopened);
}
