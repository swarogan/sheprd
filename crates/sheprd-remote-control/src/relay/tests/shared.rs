//! Relays another computer shares start and stop listening while the
//! daemon runs, with no restart.

use super::*;
use crate::control::relays::{Offer, Shared};

async fn stopped_listening(relays: &Relays) {
    for _ in 0..100 {
        if relays.status().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("still listening: {:?}", relays.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shared_relay_is_listened_on_at_once_and_dropped_when_withdrawn() {
    let (http, quic, pin) = relay().await;
    let dir = tempfile::tempdir().unwrap();
    let mut shared = state(dir.path());
    let relays = Arc::new(Relays::load(dir.path(), Vec::new()).unwrap());
    Arc::get_mut(&mut shared).unwrap().relay = Some(Arc::clone(&relays));
    let (_stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &shared).0, stopped);
    assert!(relays.info().is_none());
    assert!(relays.status().is_empty());

    let endpoint = format!("http://{http}");
    let listened = tokio::task::spawn_blocking({
        let (relays, endpoint) = (Arc::clone(&relays), endpoint.clone());
        move || {
            relays.share(Offer {
                source: "studio".into(),
                relays: vec![endpoint],
            })
        }
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(listened.len(), 1);
    listening(&relays).await;
    let info = relays.info().unwrap();
    assert_eq!(info.relays, listened);
    // A phone asks for the relay over a path it trusts, and nearby offers
    // name it.
    let (_device, token) = signed_in(&shared, 9);
    let mut request = crate::web::tests::get("/api/relay");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    assert_eq!(send(&shared, request).await.0, StatusCode::OK);
    // The route listens on the relay: a stranger hears offline, not an
    // unknown route's silence.
    let connection = client::connect(quic, pin).await.unwrap();
    let stranger = SigningKey::from_slice(&[0x77; 32]).unwrap();
    assert!(matches!(
        client::dial(&connection, relays.keys.route(), &stranger).await,
        Err(client::Error::Refused(sheprd_relay::wire::Status::Offline))
    ));

    // The share outlives a restart.
    let again = Relays::load(dir.path(), Vec::new()).unwrap();
    assert_eq!(again.endpoints().len(), 1);

    tokio::task::spawn_blocking({
        let relays = Arc::clone(&relays);
        move || {
            relays.share(Offer {
                source: "studio".into(),
                relays: Vec::new(),
            })
        }
    })
    .await
    .unwrap()
    .unwrap();
    stopped_listening(&relays).await;
    assert!(relays.info().is_none());
    let mut request = crate::web::tests::get("/api/relay");
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    assert_eq!(send(&shared, request).await.0, StatusCode::NOT_FOUND);
}

#[test]
fn configured_relays_come_first_and_shared_ones_add_only_what_is_new() {
    let dir = tempfile::tempdir().unwrap();
    let configured = Endpoint::parse("relay.example.org").unwrap();
    let mut saved = Shared::default();
    saved
        .offer(Offer {
            source: "studio".into(),
            relays: vec!["b.example.org".into(), "https://relay.example.org".into()],
        })
        .unwrap();
    super::super::shared::save(&super::super::shared::file(dir.path()), &saved).unwrap();
    let relays = Relays::load(dir.path(), vec![configured.into()]).unwrap();
    let names: Vec<String> = relays.endpoints().iter().map(ToString::to_string).collect();
    assert_eq!(
        names,
        ["https://relay.example.org", "https://b.example.org"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offer_written_after_loading_but_before_starting_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut shared = state(dir.path());
    let relays = Arc::new(Relays::load(dir.path(), Vec::new()).unwrap());
    Arc::get_mut(&mut shared).unwrap().relay = Some(Arc::clone(&relays));
    // No control socket answers yet, so the offer goes to the file.
    let path = dir.path().to_owned();
    tokio::task::spawn_blocking(move || {
        crate::relay::shared::share(
            &path,
            Offer {
                source: "studio".into(),
                relays: vec!["relay.example.org".into()],
            },
        )
    })
    .await
    .unwrap()
    .unwrap();
    assert!(relays.endpoints().is_empty());
    let (_stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &shared).0, stopped);
    assert_eq!(relays.endpoints().len(), 1);
    assert!(relays.info().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offer_written_while_the_daemon_ran_is_built_on_and_picked_up() {
    let dir = tempfile::tempdir().unwrap();
    let shared = state(dir.path());
    let relays = Arc::new(Relays::load(dir.path(), Vec::new()).unwrap());
    let (_stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &shared).0, stopped);
    let file = super::super::shared::file(dir.path());
    let offer = |source: &str, relay: &str| Offer {
        source: source.into(),
        relays: vec![relay.into()],
    };

    // A command that could not reach this daemon wrote its offer itself.
    let mut written = Shared::default();
    written.offer(offer("laptop", "a.example.org")).unwrap();
    super::super::shared::save(&file, &written).unwrap();
    // The daemon's next offer builds on the file, not its own older copy.
    let relays_for_share = Arc::clone(&relays);
    let listened = tokio::task::spawn_blocking(move || {
        relays_for_share.share(offer("studio", "b.example.org"))
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(listened.len(), 2);
    assert_eq!(super::super::shared::load(&file).endpoints().len(), 2);

    // Without one, it reads the file again on its own.
    let mut withdrawn = super::super::shared::load(&file);
    withdrawn
        .offer(Offer {
            source: "laptop".into(),
            relays: Vec::new(),
        })
        .unwrap();
    super::super::shared::save(&file, &withdrawn).unwrap();
    let relays_for_reload = Arc::clone(&relays);
    assert!(
        tokio::task::spawn_blocking(move || relays_for_reload.reload())
            .await
            .unwrap()
    );
    let names: Vec<String> = relays.endpoints().iter().map(ToString::to_string).collect();
    assert_eq!(names, ["https://b.example.org"]);
}
