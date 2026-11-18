//! Settings > Remote control > Relays: deploying a relay on DigitalOcean
//! beside adding one. With no relay listed, two cards offer both; once one
//! is, a row under Add keeps the way to deploy another.
use super::remote_control::{click, draw, page};
use super::*;
use crate::config::{preferences::Preference, remote::Edit};
use sheprd_relay::directory::DEPLOY_URL;

const HOME: &str = "https://relay.example.org";

fn with_relay(view: &Entity<SettingsWindow>, cx: &mut VisualTestContext) {
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec![HOME.into()];
        cx.notify();
    });
    draw(cx);
}

#[gpui::test]
fn with_no_relay_the_cards_offer_deploying_and_adding(cx: &mut TestAppContext) {
    let (_, cx, _) = page(cx);
    for selector in [
        "remote-relay-deploy-card",
        "remote-relay-deploy",
        "remote-relay-add",
        "remote-relay-address",
        "remote-relay-add-button",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector}");
    }
    assert!(cx.debug_bounds("remote-relay-deploy-row").is_none());
    // The cards share one line on a wide page, and the field sits in the
    // adding card, beside its button.
    let deploy = cx.debug_bounds("remote-relay-deploy-card").unwrap();
    let add = cx.debug_bounds("remote-relay-add").unwrap();
    assert_eq!(deploy.top(), add.top());
    let field = cx.debug_bounds("remote-relay-address").unwrap();
    let button = cx.debug_bounds("remote-relay-add-button").unwrap();
    assert!(add.contains(&field.center()) && add.contains(&button.center()));
    assert!(field.right() <= button.left());
}

#[gpui::test]
fn the_card_opens_the_deploy_page(cx: &mut TestAppContext) {
    let (_, cx, edits) = page(cx);
    click(cx, "remote-relay-deploy");
    assert_eq!(cx.opened_url().as_deref(), Some(DEPLOY_URL));
    assert!(edits.lock().unwrap().is_empty());
}

#[gpui::test]
fn the_cards_field_adds_a_relay(cx: &mut TestAppContext) {
    let (_, cx, edits) = page(cx);
    click(cx, "remote-relay-address");
    cx.simulate_input("relay.example.org");
    cx.run_until_parked();
    draw(cx);
    click(cx, "remote-relay-add-button");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![HOME.into()]))]
    );
}

#[gpui::test]
fn once_a_relay_is_listed_deploying_moves_under_add(cx: &mut TestAppContext) {
    let (view, cx, _) = page(cx);
    with_relay(&view, cx);
    assert!(cx.debug_bounds("remote-relay-deploy-card").is_none());
    let add = cx.debug_bounds("remote-relay-add").unwrap();
    let row = cx.debug_bounds("remote-relay-deploy-row").unwrap();
    assert!(row.top() >= add.bottom());
    click(cx, "remote-relay-deploy");
    assert_eq!(cx.opened_url().as_deref(), Some(DEPLOY_URL));
}
