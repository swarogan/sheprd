//! Settings > Remote control > Relays: the list `[remote] relays` holds,
//! how the daemon says each stands, adding or removing one saving the
//! whole list, and Sheprd's own relays offered from its signed list.
use super::remote_control::{click, draw, page, status};
use super::*;
use crate::{
    config::{preferences::Preference, remote::Edit},
    remote::{
        Health, Supervisor,
        public_relays::{self, Listed, Public},
    },
    settings_window::pages::remote_control::{Offered, Standing, public_title},
};
use sheprd_relay::directory::signed::Signer;
use sheprd_remote_control::control::RelayState;

const HOME: &str = "https://relay.example.org";
const AWAY: &str = "https://eu.example.org/relay";
/// When [`sheprd_list`] stops being taken: 2100, so tests reading the
/// clock still see it offered.
const EXPIRES: u64 = 4_102_444_800;

/// The daemon running, listening on [`HOME`] and failing on [`AWAY`].
fn listening() -> Health {
    let mut status = status();
    status.relays = [
        (
            HOME.to_owned(),
            RelayState::Listening { rtt_ms: Some(42) }.into(),
        ),
        (
            AWAY.to_owned(),
            RelayState::Failed {
                error: "the relay could not be reached".into(),
            }
            .into(),
        ),
    ]
    .into();
    Health::Running(status)
}

fn plain(line: String) -> String {
    line.replace(['\u{2068}', '\u{2069}'], "")
}

/// Shows the page with both relays configured and reported.
fn with_relays(view: &Entity<SettingsWindow>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| Supervisor::set_health(cx, listening(), None));
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec![HOME.into(), AWAY.into()];
        cx.notify();
    });
    draw(cx);
}

fn type_address(cx: &mut VisualTestContext, text: &str) {
    click(cx, "remote-relay-address");
    cx.simulate_input(text);
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn the_page_lists_no_relay_until_one_is_named(cx: &mut TestAppContext) {
    let (view, cx, _) = page(cx);
    for selector in [
        "remote-relays",
        "remote-relay-add",
        "remote-relay-address",
        "remote-relay-add-button",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }
    assert!(cx.debug_bounds("remote-relay-0").is_none());
    with_relays(&view, cx);
    for selector in [
        "remote-relay-0",
        "remote-relay-0-dot",
        "remote-relay-0-remove",
        "remote-relay-1",
        "remote-relay-1-dot",
        "remote-relay-1-remove",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }
    assert!(cx.debug_bounds("remote-relay-2").is_none());
}

#[gpui::test]
fn adding_a_relay_saves_the_list_with_it(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    with_relays(&view, cx);
    type_address(cx, " https://us.example.org/ ");
    click(cx, "remote-relay-add-button");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![
            HOME.into(),
            AWAY.into(),
            "https://us.example.org".into(),
        ]))]
    );
}

#[gpui::test]
fn what_is_not_a_relay_address_is_not_saved(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    with_relays(&view, cx);
    // Nothing typed, then plain HTTP to a public name.
    click(cx, "remote-relay-add-button");
    type_address(cx, "http://relay.example.org");
    click(cx, "remote-relay-add-button");
    assert!(edits.lock().unwrap().is_empty());
}

#[gpui::test]
fn a_relay_already_listed_is_not_added_again(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    with_relays(&view, cx);
    type_address(cx, "https://relay.example.org/");
    click(cx, "remote-relay-add-button");
    assert!(edits.lock().unwrap().is_empty());
}

#[gpui::test]
fn removing_a_relay_saves_the_list_without_it(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    with_relays(&view, cx);
    click(cx, "remote-relay-0-remove");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![AWAY.into()]))]
    );
}

#[test]
fn a_relay_row_says_how_the_daemon_stands_on_it() {
    let running = listening();
    let line = |relay: &str, health: &Health| plain(Standing::of(relay, health).line());
    assert_eq!(Standing::of(HOME, &running), Standing::Listening(Some(42)));
    assert_eq!(line(HOME, &running), "Listening · 42 ms");
    // Before its first round trip is measured, or from an older daemon.
    let mut unmeasured = status();
    unmeasured.relays = [(
        HOME.to_owned(),
        RelayState::Listening { rtt_ms: None }.into(),
    )]
    .into();
    assert_eq!(line(HOME, &Health::Running(unmeasured)), "Listening");
    assert_eq!(
        Standing::of(AWAY, &running),
        Standing::Failed("the relay could not be reached".into())
    );
    assert_eq!(
        line(AWAY, &running),
        "Failed: the relay could not be reached"
    );
    // Spelled another way by hand, it is still the relay reported.
    assert_eq!(
        Standing::of("https://relay.example.org/", &running),
        Standing::Listening(Some(42))
    );
    assert_eq!(
        line("https://new.example.org", &running),
        "The daemon has not reported on it yet."
    );
    let mut connecting = status();
    connecting.relays = [(HOME.to_owned(), RelayState::Connecting.into())].into();
    assert_eq!(
        line(HOME, &Health::Running(connecting)),
        "Connecting\u{2026}"
    );
    assert_eq!(Standing::of(HOME, &Health::Off), Standing::Off);
    assert_eq!(line(HOME, &Health::Off), "Shown while remote control runs.");
}

/// Sheprd's list with Frankfurt, which is [`HOME`], and one unnamed relay.
fn sheprd_list() -> Listed {
    Listed {
        expires: EXPIRES,
        relays: vec![
            Public {
                address: HOME.into(),
                name: Some("Frankfurt".into()),
                region: Some("fra1".into()),
            },
            Public {
                address: "https://nyc3.relay.example.org".into(),
                name: None,
                region: None,
            },
        ],
        signer: Signer::Maintainer,
    }
}

fn offer(view: &Entity<SettingsWindow>, cx: &mut VisualTestContext, offered: Offered) {
    view.update(cx, |view, cx| {
        view.remote.offered = offered;
        cx.notify();
    });
    draw(cx);
}

#[gpui::test]
fn sheprds_relays_are_offered_and_adding_one_saves_it(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    with_relays(&view, cx);
    // The page alone never reads the list in a test: nothing is offered.
    view.read_with(cx, |view, _| {
        assert_eq!(view.remote.offered, Offered::Unknown);
    });
    assert!(cx.debug_bounds("remote-public-relays").is_some());
    assert!(cx.debug_bounds("remote-public-relay-0").is_none());
    offer(&view, cx, Offered::Listed(sheprd_list()));
    for selector in [
        "remote-public-relay-0",
        "remote-public-relay-0-add",
        "remote-public-relay-1",
        "remote-public-relay-1-add",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }
    // Frankfurt is already listed, so its button does nothing.
    click(cx, "remote-public-relay-0-add");
    assert!(edits.lock().unwrap().is_empty());
    click(cx, "remote-public-relay-1-add");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![
            HOME.into(),
            AWAY.into(),
            "https://nyc3.relay.example.org".into(),
        ]))]
    );
}

#[gpui::test]
fn an_empty_or_unreadable_list_says_so(cx: &mut TestAppContext) {
    let (view, cx, _) = page(cx);
    offer(
        &view,
        cx,
        Offered::Listed(Listed {
            expires: EXPIRES,
            relays: Vec::new(),
            signer: Signer::Actions,
        }),
    );
    assert!(cx.debug_bounds("remote-public-relays-setup").is_some());
    assert!(cx.debug_bounds("remote-public-relays-retry").is_none());
    offer(&view, cx, Offered::Failed("refused".into()));
    assert!(cx.debug_bounds("remote-public-relays-retry").is_some());
    assert!(cx.debug_bounds("remote-public-relays-setup").is_none());
}

#[gpui::test]
fn the_list_is_read_off_the_ui_thread_and_its_failure_kept(cx: &mut TestAppContext) {
    let (view, cx, _) = page(cx);
    view.update(cx, |view, cx| {
        view.load_public_relays(|| Ok(sheprd_list()), cx);
        assert_eq!(view.remote.offered, Offered::Loading);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.remote.offered, Offered::Listed(sheprd_list()));
    });
    view.update(cx, |view, cx| {
        view.load_public_relays(|| Err(public_relays::Error::Missing), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.remote.offered,
            Offered::Failed("Sheprd's relay list is not published yet.".into())
        );
    });
}

#[test]
fn a_public_relay_is_titled_by_its_name_and_region() {
    let [frankfurt, unnamed] = <[Public; 2]>::try_from(sheprd_list().relays).unwrap();
    assert_eq!(plain(public_title(&frankfurt)), "Frankfurt (fra1)");
    assert_eq!(public_title(&unnamed), "https://nyc3.relay.example.org");
    let region_only = Public {
        name: None,
        ..frankfurt
    };
    assert_eq!(public_title(&region_only), "fra1");
}

#[gpui::test]
fn an_expired_list_stops_offering_its_relays(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    offer(&view, cx, Offered::Listed(sheprd_list()));
    // Still good a second before it expires: Add saves.
    view.update(cx, |view, cx| {
        view.add_public_relay_at(1, EXPIRES - 1, cx);
    });
    cx.run_until_parked();
    assert_eq!(edits.lock().unwrap().len(), 1);
    // The window stayed open past the expiry: Add saves nothing, the
    // relays go, and Retry reads the list again.
    view.update(cx, |view, cx| view.add_public_relay_at(1, EXPIRES, cx));
    cx.run_until_parked();
    draw(cx);
    assert_eq!(edits.lock().unwrap().len(), 1);
    assert!(cx.debug_bounds("remote-public-relay-0").is_none());
    assert!(cx.debug_bounds("remote-public-relays-retry").is_some());
    view.read_with(cx, |view, _| {
        assert!(matches!(view.remote.offered, Offered::Failed(_)));
    });
    // Coming back to the page with an expired list does the same.
    let mut old = sheprd_list();
    old.expires = 1;
    offer(&view, cx, Offered::Listed(old));
    view.update(cx, |view, cx| view.open_remote_control(cx));
    draw(cx);
    assert!(cx.debug_bounds("remote-public-relay-0").is_none());
    view.read_with(cx, |view, _| {
        assert!(matches!(view.remote.offered, Offered::Failed(_)));
    });
}
