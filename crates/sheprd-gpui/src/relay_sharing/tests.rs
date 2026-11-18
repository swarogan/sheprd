use super::*;

fn relays(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_machine_never_offered_hears_at_once() {
    let now = Instant::now();
    assert!(Link::default().due(&relays(&["relay.example.org"]), now));
    // An empty list too: it withdraws what an earlier run shared.
    assert!(Link::default().due(&[], now));
}

#[test]
fn a_change_goes_at_once_and_the_same_list_only_now_and_then() {
    let now = Instant::now();
    let list = relays(&["relay.example.org"]);
    let link = Link {
        sent: Some(list.clone()),
        last: Some(now),
        running: false,
    };
    assert!(!link.due(&list, now));
    assert!(!link.due(&list, now + RETRY));
    assert!(link.due(&list, now + REFRESH));
    assert!(link.due(&relays(&["b.example.org"]), now));
    assert!(link.due(&[], now));
}

#[test]
fn an_empty_list_is_not_offered_again_once_taken() {
    let now = Instant::now();
    let link = Link {
        sent: Some(Vec::new()),
        last: Some(now),
        running: false,
    };
    assert!(!link.due(&[], now + REFRESH * 10));
}

#[test]
fn a_machine_that_did_not_take_them_waits_before_another_try() {
    let now = Instant::now();
    let list = relays(&["relay.example.org"]);
    let link = Link {
        sent: None,
        last: Some(now),
        running: false,
    };
    assert!(!link.due(&list, now + RETRY / 2));
    assert!(link.due(&list, now + RETRY));
}

#[test]
fn one_offer_at_a_time() {
    let link = Link {
        running: true,
        ..Link::default()
    };
    assert!(!link.due(&relays(&["relay.example.org"]), Instant::now()));
}

#[test]
fn the_host_answer_says_whether_it_took_them() {
    assert!(sync::parse(br#"{"relays":["https://relay.example.org"]}"#).unwrap());
    // No program, or one older than shared relays.
    assert!(!sync::parse(b"").unwrap());
    assert!(!sync::parse(b" \n").unwrap());
    assert!(matches!(sync::parse(b"nope"), Err(Error::Answer(_))));
    assert!(sync::SCRIPT.contains("relays --offer"));
}

fn target(host: &str) -> ConnectTarget {
    ConnectTarget::Ssh {
        target: host.into(),
        session: "default".into(),
    }
}

fn reached(hosts: &[&str], at: Instant) -> Reached {
    Reached {
        machines: hosts
            .iter()
            .map(|host| ((*host).to_owned(), target(host)))
            .collect(),
        at,
    }
}

#[test]
fn each_window_counts_and_one_never_hides_another_s_machines() {
    let now = Instant::now();
    let (one, two) = (EntityId::from(1_u64), EntityId::from(2_u64));
    let mut sharing = RelaySharing::default();
    sharing.windows.insert(one, reached(&["ssh:a"], now));
    sharing.windows.insert(two, reached(&["ssh:b"], now));
    sharing.links.insert(
        "ssh:a".into(),
        Link {
            sent: Some(relays(&["relay.example.org"])),
            last: Some(now),
            running: false,
        },
    );
    sharing.forget_unreached(now);
    let machines: Vec<String> = sharing.machines(now).into_keys().collect();
    assert_eq!(machines, ["ssh:a", "ssh:b"]);
    // Window two's report keeps window one's link as it was.
    assert!(sharing.links["ssh:a"].sent.is_some());
}

#[test]
fn a_silent_window_stops_counting_and_its_machines_hear_again_on_return() {
    let start = Instant::now();
    let later = start + WINDOW_SILENCE;
    let (closed, open) = (EntityId::from(1_u64), EntityId::from(2_u64));
    let mut sharing = RelaySharing::default();
    sharing
        .windows
        .insert(closed, reached(&["ssh:a", "ssh:b"], start));
    sharing.windows.insert(open, reached(&[], later));
    for (id, running) in [("ssh:a", false), ("ssh:b", true)] {
        sharing.links.insert(
            id.into(),
            Link {
                sent: Some(Vec::new()),
                last: Some(start),
                running,
            },
        );
    }
    sharing.forget_unreached(later);
    assert!(!sharing.windows.contains_key(&closed));
    assert!(sharing.machines(later).is_empty());
    // A link with an offer running is kept, so it never starts twice.
    assert!(!sharing.links.contains_key("ssh:a"));
    assert!(sharing.links["ssh:b"].running);
}
