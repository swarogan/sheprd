use super::*;
use serde_json::json;

#[test]
fn a_new_pairing_shares_github_unless_the_approval_says_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut auth = auth(dir.path());
    let shared = pair(&mut auth, &signing_key(1), T0);
    assert!(auth.devices().get(&shared).unwrap().share_github);

    let offer = auth.mint(T0, &origins(), FINGERPRINT).unwrap();
    let claim = auth
        .claim(
            IpAddr::from([192, 168, 1, 31]),
            &token(&offer),
            "iPad",
            &public_key(&signing_key(2)),
            T0,
        )
        .unwrap();
    let PairingState::Approved { device } = auth
        .answer_with_hosts(&offer.id, true, &claim.code, &[], false, T0)
        .unwrap()
    else {
        panic!("not approved");
    };
    assert!(!auth.devices().get(&device).unwrap().share_github);
    let listed = auth.device_list(|_| false, T0);
    assert_eq!(
        listed
            .iter()
            .map(|device| device.share_github)
            .collect::<Vec<_>>(),
        [true, false]
    );
    // The choice is on disk.
    let reloaded = Devices::load(dir.path()).unwrap();
    assert!(!reloaded.get(&device).unwrap().share_github);
}

#[test]
fn a_record_from_before_the_switch_shares_github() {
    let dir = tempfile::tempdir().unwrap();
    let key = public_key(&signing_key(1));
    let store = json!({
        "version": 1,
        "devices": [{
            "id": "old",
            "name": "iPhone",
            "public_key": key,
            "created_ms": T0,
        }],
    });
    std::fs::write(dir.path().join("devices.json"), store.to_string()).unwrap();
    let devices = Devices::load(dir.path()).unwrap();
    assert!(devices.get(&DeviceId("old".into())).unwrap().share_github);
    // An older Sheprd's device list reads the same way.
    let listed: control::Device = serde_json::from_value(json!({
        "id": "old",
        "name": "iPhone",
        "created_ms": T0,
        "last_seen_ms": null,
        "push": false,
        "connected": false,
    }))
    .unwrap();
    assert!(listed.share_github);
}

#[test]
fn the_switch_is_saved_and_refused_for_an_unknown_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut auth = auth(dir.path());
    let device = pair(&mut auth, &signing_key(1), T0);
    assert!(auth.devices_mut().set_share_github(&device, false).unwrap());
    assert!(
        !Devices::load(dir.path())
            .unwrap()
            .get(&device)
            .unwrap()
            .share_github
    );
    assert!(
        !auth
            .devices_mut()
            .set_share_github(&DeviceId("gone".into()), true)
            .unwrap()
    );
}
