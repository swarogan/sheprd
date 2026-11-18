//! Relays listed nearest first, with the round trips the phone adds its
//! own leg to.

use super::*;
use std::collections::BTreeMap;

/// The relays to listen on, unlabelled.
fn listened(endpoints: &[Endpoint]) -> Vec<Relay> {
    endpoints.iter().cloned().map(Relay::from).collect()
}

fn endpoints(names: &[&str]) -> Vec<Endpoint> {
    names
        .iter()
        .map(|name| Endpoint::parse(name).unwrap())
        .collect()
}

#[test]
fn measured_relays_come_first_fastest_first_and_the_rest_keep_their_order() {
    let all = endpoints(&[
        "https://us-west.example.org",
        "https://europe.example.org",
        "https://asia.example.org",
        "https://us-east.example.org",
    ]);
    let rtt = BTreeMap::from([
        (
            "https://asia.example.org".to_owned(),
            Duration::from_millis(180),
        ),
        (
            "https://europe.example.org".to_owned(),
            Duration::from_millis(12),
        ),
    ]);
    assert_eq!(
        crate::relay::nearest::first(&all, &rtt),
        [
            "https://europe.example.org",
            "https://asia.example.org",
            "https://us-west.example.org",
            "https://us-east.example.org",
        ]
    );
    assert_eq!(
        crate::relay::nearest::millis(&rtt),
        BTreeMap::from([
            ("https://asia.example.org".to_owned(), 180),
            ("https://europe.example.org".to_owned(), 12),
        ])
    );
}

#[test]
fn a_sub_millisecond_round_trip_reads_as_one() {
    let rtt = BTreeMap::from([(
        "https://a.example.org".to_owned(),
        Duration::from_micros(80),
    )]);
    assert_eq!(
        crate::relay::nearest::millis(&rtt)["https://a.example.org"],
        1
    );
}

#[test]
fn a_relay_that_stopped_listening_is_no_longer_offered_as_nearest() {
    let dir = tempfile::tempdir().unwrap();
    let all = endpoints(&["https://far.example.org", "https://near.example.org"]);
    let relays = Relays::load(dir.path(), listened(&all)).unwrap();
    relays.set_rtt(&all[0], Some(Duration::from_millis(200)));
    relays.set_rtt(&all[1], Some(Duration::from_millis(20)));
    assert_eq!(relays.info().unwrap().relays[0], "https://near.example.org");
    relays.set_rtt(&all[1], None);
    let info = relays.info().unwrap();
    assert_eq!(
        info.relays,
        ["https://far.example.org", "https://near.example.org"]
    );
    assert_eq!(info.rtt_ms.len(), 1);
    let json = serde_json::to_value(&info).unwrap();
    assert_eq!(json["rtt_ms"]["https://far.example.org"], 200);
}

#[test]
fn a_computer_listening_nowhere_sends_no_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let relays =
        Relays::load(dir.path(), listened(&endpoints(&["https://a.example.org"]))).unwrap();
    let json = serde_json::to_value(relays.info().unwrap()).unwrap();
    assert!(json.get("rtt_ms").is_none());
}

#[test]
fn a_nearby_offer_carries_the_round_trips_measured_since_start() {
    let dir = tempfile::tempdir().unwrap();
    let all = endpoints(&["https://far.example.org", "https://near.example.org"]);
    let relays = Relays::load(dir.path(), listened(&all)).unwrap();
    let mut auth = crate::auth::Auth::load(dir.path(), crate::auth::Limits::default()).unwrap();
    // At start nothing listens yet.
    refresh_offer(Some(&relays), &mut auth);
    relays.set_rtt(&all[1], Some(Duration::from_millis(15)));
    refresh_offer(Some(&relays), &mut auth);
    let offer = serde_json::to_value(crate::auth::nearby::Offer {
        token: "t".into(),
        origins: &[],
        relay: auth.relay_offer(),
    })
    .unwrap();
    assert_eq!(offer["relay"]["relays"][0], "https://near.example.org");
    assert_eq!(offer["relay"]["rtt_ms"]["https://near.example.org"], 15);
}

#[test]
fn status_reports_a_listening_relays_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let all = endpoints(&["https://a.example.org", "https://b.example.org"]);
    let relays = Relays::load(dir.path(), listened(&all)).unwrap();
    // Only a running listener's state is recorded.
    for endpoint in &all {
        relays
            .running
            .lock()
            .unwrap()
            .listeners
            .insert(endpoint.to_string(), watch::Sender::new(false));
    }
    relays.set_status(&all[0], Status::Listening { rtt_ms: None });
    relays.set_status(&all[1], Status::Connecting);
    assert_eq!(
        relays.status()["https://a.example.org"].state,
        Status::Listening { rtt_ms: None }
    );
    relays.set_rtt(&all[0], Some(Duration::from_millis(42)));
    relays.set_rtt(&all[1], Some(Duration::from_millis(7)));
    let status = relays.status();
    assert_eq!(
        status["https://a.example.org"].state,
        Status::Listening { rtt_ms: Some(42) }
    );
    assert_eq!(status["https://b.example.org"].state, Status::Connecting);
}

/// The state as an app from before round trips declares it.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
enum OlderState {
    Connecting,
    Listening,
    Failed { error: String },
}

#[test]
fn the_round_trip_reads_in_older_apps_and_from_older_daemons() {
    let newer = serde_json::to_string(&Status::Listening { rtt_ms: Some(42) }).unwrap();
    assert_eq!(newer, r#"{"state":"listening","rtt_ms":42}"#);
    assert_eq!(
        serde_json::from_str::<OlderState>(&newer).unwrap(),
        OlderState::Listening
    );
    assert_eq!(
        serde_json::from_str::<Status>(r#"{"state":"listening"}"#).unwrap(),
        Status::Listening { rtt_ms: None }
    );
    assert_eq!(
        serde_json::to_string(&Status::Listening { rtt_ms: None }).unwrap(),
        r#"{"state":"listening"}"#
    );
}
