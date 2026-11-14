//! A relay whose name resolves to several addresses is reached at the
//! first that answers.

use super::*;
use crate::relay::listen::first_reachable;
use std::sync::Mutex;

const ATTEMPT: Duration = Duration::from_millis(50);

fn addresses() -> [SocketAddr; 3] {
    [
        "[2001:db8::1]:443".parse().unwrap(),
        "192.0.2.1:443".parse().unwrap(),
        "192.0.2.2:443".parse().unwrap(),
    ]
}

#[tokio::test]
async fn an_address_that_fails_gives_way_to_the_next() {
    let [v6, v4, _] = addresses();
    let tried = Mutex::new(Vec::new());
    let reached = first_reachable(&addresses(), ATTEMPT, |address| {
        tried.lock().unwrap().push(address);
        async move {
            if address == v6 {
                Err(Error::NoQuic)
            } else {
                Ok(address)
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(reached, v4);
    // Nothing past the one that answered is tried.
    assert_eq!(*tried.lock().unwrap(), [v6, v4]);
}

#[tokio::test]
async fn an_address_that_never_answers_gives_way_to_the_next() {
    let [v6, v4, _] = addresses();
    let reached = first_reachable(&[v6, v4], ATTEMPT, |address| async move {
        if address == v6 {
            std::future::pending::<()>().await;
        }
        Ok(address)
    })
    .await
    .unwrap();
    assert_eq!(reached, v4);
}

#[tokio::test]
async fn the_last_address_failing_is_the_failure_reported() {
    let [_, _, last] = addresses();
    let failed = first_reachable(&addresses(), ATTEMPT, |address| async move {
        if address == last {
            Err::<(), _>(Error::InfoFormat)
        } else {
            Err(Error::NoQuic)
        }
    })
    .await;
    assert!(matches!(failed, Err(Error::InfoFormat)));
}

#[tokio::test]
async fn a_name_with_no_address_did_not_resolve() {
    let failed = first_reachable(&[], ATTEMPT, |_| async { Ok(()) }).await;
    assert!(matches!(failed, Err(Error::Resolve(_))));
}
