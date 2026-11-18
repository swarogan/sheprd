//! What `[remote] relays` calls each relay reaches paired phones and
//! `sheprd remote status`, and stays out of the nearby offer.

use super::*;
use crate::relay_label::RelayLabel;

fn fra3() -> RelayLabel {
    RelayLabel {
        description: Some("not for public use".into()),
        name: Some("fra3".into()),
        tags: vec!["eu".into()],
    }
}

fn labelled(dir: &Path) -> Relays {
    let sfo3 = Endpoint::parse("https://sfo3.example.org").unwrap();
    let fra3_endpoint = Endpoint::parse("https://fra3.example.org").unwrap();
    Relays::load(
        dir,
        vec![
            sfo3.into(),
            Relay {
                endpoint: fra3_endpoint,
                label: fra3(),
            },
        ],
    )
    .unwrap()
}

#[test]
fn the_phone_learns_each_labelled_relays_label() {
    let dir = tempfile::tempdir().unwrap();
    let info = serde_json::to_value(labelled(dir.path()).info().unwrap()).unwrap();
    assert_eq!(
        info["relays"],
        serde_json::json!(["https://sfo3.example.org", "https://fra3.example.org"])
    );
    assert_eq!(
        info["labels"],
        serde_json::json!({
            "https://fra3.example.org": {
                "description": "not for public use", "name": "fra3", "tags": ["eu"],
            },
        })
    );
    // Without any label the field is left out, as older daemons send it.
    let plain = Relays::load(
        dir.path(),
        vec![Endpoint::parse("https://sfo3.example.org").unwrap().into()],
    )
    .unwrap();
    assert!(
        serde_json::to_value(plain.info().unwrap())
            .unwrap()
            .get("labels")
            .is_none()
    );
}

#[test]
fn the_nearby_offer_leaves_the_labels_out() {
    let dir = tempfile::tempdir().unwrap();
    let relays = labelled(dir.path());
    let offer = serde_json::to_value(relays.offer().unwrap()).unwrap();
    assert!(offer.get("labels").is_none());
    assert_eq!(offer["relays"].as_array().map(Vec::len), Some(2));
    // Nor when it is refreshed before each offer is sealed.
    let mut auth = crate::auth::Auth::load(dir.path(), crate::auth::Limits::default()).unwrap();
    refresh_offer(Some(&relays), &mut auth);
    let sealed = serde_json::to_value(crate::auth::nearby::Offer {
        token: "t".into(),
        origins: &[],
        relay: auth.relay_offer(),
    })
    .unwrap();
    assert!(sealed["relay"].get("labels").is_none(), "{sealed}");
    assert_eq!(sealed["relay"]["relays"].as_array().map(Vec::len), Some(2));
}

#[test]
fn status_names_each_relay_with_its_label() {
    let dir = tempfile::tempdir().unwrap();
    let relays = labelled(dir.path());
    for endpoint in relays.endpoints() {
        // Only a running listener's state is recorded.
        relays
            .running
            .lock()
            .unwrap()
            .listeners
            .insert(endpoint.to_string(), watch::Sender::new(false));
        relays.set_status(&endpoint, Status::Listening { rtt_ms: None });
    }
    let status = relays.status();
    assert_eq!(
        status["https://fra3.example.org"],
        RelayStatus {
            label: fra3(),
            state: Status::Listening { rtt_ms: None },
        }
    );
    assert_eq!(
        status["https://sfo3.example.org"],
        Status::Listening { rtt_ms: None }.into()
    );
}
