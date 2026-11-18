use super::*;

fn offer(source: &str, relays: &[&str]) -> Offer {
    Offer {
        source: source.into(),
        relays: relays.iter().map(|relay| (*relay).to_owned()).collect(),
    }
}

fn names(shared: &Shared) -> Vec<String> {
    shared.endpoints().iter().map(ToString::to_string).collect()
}

#[test]
fn an_offer_replaces_what_its_source_shared_before_and_leaves_others() {
    let mut shared = Shared::default();
    assert!(
        shared
            .offer(offer("studio", &["relay.example.org", "b.example.org"]))
            .unwrap()
    );
    assert!(shared.offer(offer("laptop", &["c.example.org"])).unwrap());
    assert!(shared.offer(offer("studio", &["b.example.org"])).unwrap());
    assert_eq!(
        names(&shared),
        ["https://c.example.org", "https://b.example.org"]
    );
    // The same relays again change nothing.
    assert!(
        !shared
            .offer(offer("studio", &["https://b.example.org"]))
            .unwrap()
    );
}

#[test]
fn an_empty_offer_withdraws_its_source() {
    let mut shared = Shared::default();
    shared
        .offer(offer("studio", &["relay.example.org"]))
        .unwrap();
    assert!(shared.offer(offer("studio", &[])).unwrap());
    assert!(shared.endpoints().is_empty());
    assert!(!shared.offer(offer("studio", &[])).unwrap());
}

#[test]
fn relays_named_twice_are_listened_on_once() {
    let mut shared = Shared::default();
    shared
        .offer(offer(
            "studio",
            &["relay.example.org", "wss://relay.example.org/"],
        ))
        .unwrap();
    shared
        .offer(offer("laptop", &["https://relay.example.org"]))
        .unwrap();
    assert_eq!(names(&shared), ["https://relay.example.org"]);
}

#[test]
fn a_bad_offer_is_refused_whole() {
    let mut shared = Shared::default();
    shared
        .offer(offer("studio", &["relay.example.org"]))
        .unwrap();
    let refused = shared.offer(offer("studio", &["b.example.org", "http://203.0.113.9"]));
    assert!(matches!(refused, Err(Error::Endpoint { .. })));
    assert!(matches!(
        shared.offer(offer(" ", &["b.example.org"])),
        Err(Error::Source)
    ));
    assert_eq!(names(&shared), ["https://relay.example.org"]);
}

#[test]
fn sources_and_their_relays_are_bounded() {
    let mut shared = Shared::default();
    for source in 0..MAX_SOURCES {
        shared
            .offer(offer(&format!("s{source}"), &["relay.example.org"]))
            .unwrap();
    }
    let refused = shared.offer(offer("one-more", &["relay.example.org"]));
    assert!(matches!(refused, Err(Error::TooManySources)));
    // A source already known may still change what it shares.
    assert!(shared.offer(offer("s0", &["b.example.org"])).unwrap());

    let many: Vec<String> = (0..MAX_RELAYS + 4)
        .map(|n| format!("r{n}.example.org"))
        .collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let mut one = Shared::default();
    one.offer(offer("studio", &many)).unwrap();
    assert_eq!(one.endpoints().len(), MAX_RELAYS);
}
