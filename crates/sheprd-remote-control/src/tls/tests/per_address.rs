use super::*;

fn certificate(resolver: &Resolver, address: &str) -> Vec<u8> {
    resolver.current_for(ip(address)).unwrap().cert[0].to_vec()
}

#[test]
fn changing_lan_does_not_replace_tailscale_or_dns_certificates() {
    let state = tempfile::tempdir().unwrap();
    let authority = Authority::load_or_create(state.path()).unwrap();
    let now = SystemTime::now();
    let original = Names::new(
        [
            ip("192.168.1.20"),
            ip("100.64.0.3"),
            ip("fd7a:115c:a1e0::3"),
        ],
        Some("studio"),
    );
    let resolver = Resolver::issue(&authority, original.clone(), now).unwrap();
    let v4 = certificate(&resolver, "100.64.0.3");
    let v6 = certificate(&resolver, "fd7a:115c:a1e0::3");
    let lan = certificate(&resolver, "192.168.1.20");
    let dns = resolver.current().cert[0].to_vec();
    assert_ne!(v4, v6);
    let changed = Names::new(
        [ip("10.1.2.3"), ip("100.64.0.3"), ip("fd7a:115c:a1e0::3")],
        Some("studio"),
    );
    assert!(resolver.refresh(&authority, &changed, now + DAY).unwrap());
    assert_eq!(certificate(&resolver, "100.64.0.3"), v4);
    assert_eq!(certificate(&resolver, "fd7a:115c:a1e0::3"), v6);
    assert_eq!(resolver.current().cert[0].as_ref(), dns);
    assert!(resolver.current_for(ip("192.168.1.20")).is_none());
    verify(
        &authority,
        &resolver.current_for(ip("10.1.2.3")).unwrap(),
        "10.1.2.3",
        now + DAY,
    )
    .unwrap();
    let restarted = Resolver::issue(
        &Authority::load_or_create(state.path()).unwrap(),
        changed,
        now + DAY,
    )
    .unwrap();
    assert_eq!(certificate(&restarted, "100.64.0.3"), v4);
    assert_eq!(certificate(&restarted, "fd7a:115c:a1e0::3"), v6);
    // Returning to a previous LAN restores its saved identity too.
    restarted.refresh(&authority, &original, now + DAY).unwrap();
    assert_eq!(certificate(&restarted, "192.168.1.20"), lan);
}

#[test]
fn migration_keeps_the_legacy_certificate_for_addresses_it_already_covers() {
    let state = tempfile::tempdir().unwrap();
    let authority = Authority::load_or_create(state.path()).unwrap();
    let now = SystemTime::now();
    let names = lan_names();
    let legacy = authority.issue(&names, now).unwrap().cert[0].to_vec();
    let resolver = Resolver::issue(&authority, names.clone(), now).unwrap();
    for address in &names.ips {
        assert_eq!(
            resolver.current_for(*address).unwrap().cert[0].as_ref(),
            legacy
        );
    }
    let expanded = Names::new(
        names.ips.into_iter().chain([ip("10.9.8.7")]),
        Some("studio"),
    );
    resolver.refresh(&authority, &expanded, now + DAY).unwrap();
    assert_eq!(certificate(&resolver, "100.101.102.103"), legacy);
    assert_ne!(certificate(&resolver, "10.9.8.7"), legacy);
}

#[test]
fn each_address_renews_only_when_its_own_certificate_needs_it() {
    let state = tempfile::tempdir().unwrap();
    let authority = Authority::load_or_create(state.path()).unwrap();
    let now = SystemTime::now();
    let resolver = Resolver::issue(&authority, lan_names(), now).unwrap();
    let tailscale = certificate(&resolver, "100.101.102.103");
    let added = Names::new([ip("100.101.102.103"), ip("10.1.2.3")], Some("studio"));
    resolver
        .refresh(&authority, &added, now + 30 * DAY)
        .unwrap();
    let newer_lan = certificate(&resolver, "10.1.2.3");
    resolver
        .refresh(&authority, &added, now + 85 * DAY)
        .unwrap();
    assert_ne!(certificate(&resolver, "100.101.102.103"), tailscale);
    assert_eq!(certificate(&resolver, "10.1.2.3"), newer_lan);
}

#[test]
fn a_single_address_bind_still_has_the_relay_sessions_loopback_certificate() {
    let state = tempfile::tempdir().unwrap();
    let authority = Authority::load_or_create(state.path()).unwrap();
    let now = SystemTime::now();
    let advertised = lan_names().bound_to(ip("192.168.1.20"));
    // Bound to the LAN address alone, loopback is neither advertised nor,
    // without relays, given a certificate.
    assert!(!advertised.ips.contains(&ip("127.0.0.1")));
    let certified = advertised.clone().with_relay_loopback();
    assert_eq!(certified.ips, [ip("127.0.0.1"), ip("192.168.1.20")]);
    assert_eq!(advertised.origins(7443), ["https://192.168.1.20:7443"]);

    let resolver = Arc::new(Resolver::issue(&authority, certified.clone(), now).unwrap());
    // What a relay session presents: no name sent, reached at loopback.
    let presented = resolver.for_handshake(ip("127.0.0.1"), false);
    verify(&authority, &presented, "127.0.0.1", now).unwrap();
    let loopback = certificate(&resolver, "127.0.0.1");

    // An address refresh keeps it, and the same one.
    resolver.refresh(&authority, &certified, now + DAY).unwrap();
    assert_eq!(certificate(&resolver, "127.0.0.1"), loopback);
    // Without it, the session would fall back to a certificate the phone
    // refuses for `127.0.0.1`.
    let unrelayed = Resolver::issue(&authority, advertised, now).unwrap();
    assert!(unrelayed.current_for(ip("127.0.0.1")).is_none());
    assert!(
        verify(
            &authority,
            &unrelayed.for_handshake(ip("127.0.0.1"), false),
            "127.0.0.1",
            now
        )
        .is_err()
    );
}
