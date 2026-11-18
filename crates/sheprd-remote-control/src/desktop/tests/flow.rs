//! The whole flow on a real relay on loopback. Unix only: the computer
//! being added must be one whose `authorized_keys` desktops can be let
//! into, and whose user `Login` names from `USER`.

use super::*;
use crate::{
    control::PairingState,
    relay::{
        Relays,
        tests::{listening, relay, served},
    },
    ssh_access::{AuthorizedKeys, SshAccess},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sheprd_relay::Endpoint;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::watch,
};

/// A stand-in SSH server that greets each connection, then echoes.
async fn ssh_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                socket.write_all(b"SSH-2.0-test\r\n").await.unwrap();
                let mut buffer = [0; 1024];
                while let Ok(read) = socket.read(&mut buffer).await {
                    if read == 0 || socket.write_all(&buffer[..read]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    address
}

/// The other computer: its daemon listening on a relay on loopback, with
/// Pair device open and desktops let in to a stand-in SSH server.
struct Other {
    dir: tempfile::TempDir,
    pairing: crate::control::PairingId,
    state: Shared,
    _stop: watch::Sender<bool>,
}

async fn other() -> Other {
    let (http, _, _) = relay().await;
    let dir = tempfile::tempdir().unwrap();
    let mut state = state(dir.path());
    let endpoint = Endpoint::parse(&format!("http://{http}")).unwrap();
    let relays = Arc::new(Relays::load(dir.path(), vec![endpoint.into()]).unwrap());
    Arc::get_mut(&mut state).unwrap().relay = Some(Arc::clone(&relays));
    let host_key_dir = dir.path().join("etc-ssh");
    std::fs::create_dir(&host_key_dir).unwrap();
    std::fs::write(
        host_key_dir.join("ssh_host_ed25519_key.pub"),
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBERERERERERERERERERERERERERERERERERERERERER root@other\n",
    )
    .unwrap();
    let server = ssh_server().await;
    state.auth().set_ssh_access(SshAccess {
        host_key_dir,
        keys: AuthorizedKeys::at(dir.path().join("authorized_keys")),
        server,
    });
    let (stop, stopped) = watch::channel(false);
    relays.start(served(dir.path(), &state).0, stopped);
    listening(&relays).await;
    let pairing = state
        .auth()
        .mint(
            now_ms(),
            &[crate::web::tests::ORIGIN.to_owned()],
            &state.fingerprint,
        )
        .unwrap()
        .id;
    Other {
        dir,
        pairing,
        state,
        _stop: stop,
    }
}

/// The other computer's user, once asked, allows the claim with the code
/// its dialog shows.
async fn allow(other: &Other) {
    for _ in 0..200 {
        let pairing = other
            .state
            .auth()
            .pairing(&other.pairing, now_ms())
            .unwrap();
        if let PairingState::Claimed { code, ssh, .. } = pairing {
            assert!(ssh, "the dialog must say a desktop wants SSH");
            other
                .state
                .auth()
                .answer_with_hosts(&other.pairing, true, &code, &[], true, now_ms())
                .unwrap();
            // As the control socket's approval does.
            if let Some(relay) = &other.state.relay {
                relay.devices_changed();
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never claimed");
}

/// This computer's side up to the opened offer.
fn exchange_with(other: &Other) -> Opened {
    let (mut exchange, start) = exchange::Exchange::start().unwrap();
    let reveal = exchange.reveal(&bluetooth(&other.state, &start)).unwrap();
    exchange.open(&bluetooth(&other.state, &reveal)).unwrap()
}

/// An `ssh-ed25519` public key line.
fn ssh_key() -> String {
    let bytes = [
        &11u32.to_be_bytes()[..],
        b"ssh-ed25519",
        &32u32.to_be_bytes(),
        &[0x22; 32],
    ]
    .concat();
    format!("ssh-ed25519 {}", STANDARD.encode(bytes))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_nearby_desktop_off_the_network_pairs_and_reaches_ssh_through_a_relay() {
    let other = other().await;
    let here = tempfile::tempdir().unwrap();
    let keys = Keys::load_or_create(here.path()).unwrap();
    let opened = exchange_with(&other);
    assert_eq!(opened.computer, "Studio");

    let ssh_key = ssh_key();
    let claimant = claim::Claimant {
        keys: &keys,
        name: "Laptop",
        ssh_key: &ssh_key,
    };
    let (allowed, ()) = tokio::join!(claim::claim(&opened, &claimant, || {}), allow(&other));
    let allowed = allowed.unwrap();
    assert_eq!(
        allowed.login.host_keys,
        ["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBERERERERERERERERERERERERERERERERERERERERER"]
    );

    // The key went in, restricted to loopback, named for the device.
    let authorized = std::fs::read_to_string(other.dir.path().join("authorized_keys")).unwrap();
    assert!(
        authorized.starts_with(r#"from="127.0.0.1,::1""#),
        "{authorized}"
    );
    assert!(authorized.contains(&ssh_key), "{authorized}");

    // Saved, it is reached through the relay, inside the tunnel, at the
    // SSH server.
    let id = allowed
        .record
        .save(here.path(), &allowed.login.host_keys)
        .unwrap();
    let known = std::fs::read_to_string(sheprd_client::relay_host::known_hosts(
        here.path(),
        &sheprd_client::relay_host::host_name(&id),
    ))
    .unwrap();
    assert!(known.starts_with(&format!(
        "{} ssh-ed25519 ",
        sheprd_client::relay_host::host_name(&id)
    )));
    let (_connection, mut tunnel) =
        connect::open(here.path(), &sheprd_client::relay_host::host_name(&id))
            .await
            .unwrap();
    let mut greeting = [0; 14];
    tokio::time::timeout(Duration::from_secs(5), tunnel.io.read_exact(&mut greeting))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&greeting, b"SSH-2.0-test\r\n");

    // Revoked, its line is gone, and its open session ends here even
    // though the relay was never told to drop its dial key.
    let device = other.state.auth().devices().records()[0].id.clone();
    other.state.auth().revoke(&device).unwrap();
    let authorized = std::fs::read_to_string(other.dir.path().join("authorized_keys")).unwrap();
    assert!(!authorized.contains(&ssh_key), "{authorized}");
    let mut rest = Vec::new();
    let ended =
        tokio::time::timeout(Duration::from_secs(5), tunnel.io.read_to_end(&mut rest)).await;
    assert!(ended.is_ok(), "the session outlived the revoked desktop");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_denied_desktop_learns_so_and_gets_no_key_in() {
    let other = other().await;
    let here = tempfile::tempdir().unwrap();
    let keys = Keys::load_or_create(here.path()).unwrap();
    let opened = exchange_with(&other);
    let ssh_key = ssh_key();
    let claimant = claim::Claimant {
        keys: &keys,
        name: "Laptop",
        ssh_key: &ssh_key,
    };
    let deny = async {
        for _ in 0..200 {
            if matches!(
                other
                    .state
                    .auth()
                    .pairing(&other.pairing, now_ms())
                    .unwrap(),
                PairingState::Claimed { .. }
            ) {
                other
                    .state
                    .auth()
                    .answer_with_hosts(&other.pairing, false, "", &[], true, now_ms())
                    .unwrap();
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let (result, ()) = tokio::join!(claim::claim(&opened, &claimant, || {}), deny);
    assert!(matches!(result, Err(Error::Denied)), "{result:?}");
    assert!(!other.dir.path().join("authorized_keys").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_claim_is_refused_before_its_token_is_spent_while_ssh_is_off() {
    let other = other().await;
    // Nothing listens on a port just freed.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let access = other.state.auth().ssh_access().unwrap().clone();
    other.state.auth().set_ssh_access(SshAccess {
        server: closed,
        ..access
    });
    let here = tempfile::tempdir().unwrap();
    let keys = Keys::load_or_create(here.path()).unwrap();
    let opened = exchange_with(&other);
    let ssh_key = ssh_key();
    let claimant = claim::Claimant {
        keys: &keys,
        name: "Laptop",
        ssh_key: &ssh_key,
    };
    let result = claim::claim(&opened, &claimant, || {}).await;
    assert!(
        matches!(&result, Err(Error::Claim(reason)) if reason.contains("Remote Login")),
        "{result:?}"
    );
    assert!(matches!(
        other
            .state
            .auth()
            .pairing(&other.pairing, now_ms())
            .unwrap(),
        PairingState::Waiting { .. }
    ));
}
