//! `herdr-remote-control relay-enroll`, which Sheprd on another computer
//! runs on this host over SSH to carry a phone's relay keys here
//! (`control::host_relay`). It reads one signed request from standard input
//! and hands it to the running daemon, which checks the signature against
//! the phone's own key and keeps the keys; it never starts the daemon, as
//! only a host that already holds the phone has keys to keep.

use crate::control::{
    self, Request, Response,
    host_relay::{self, Outcome},
};
use std::path::Path;

/// Hands `request` to the daemon whose state is in `state_dir`; what it
/// came to, failures included, for Sheprd to tell the phone. Blocking.
pub fn relay_enroll(request: &host_relay::Request, state_dir: &Path) -> Outcome {
    let address = control::address(state_dir);
    let failed = |reason: &str| Outcome::Failed {
        reason: reason.to_owned(),
    };
    match control::request(&address, &Request::RelayEnroll(Box::new(request.clone()))) {
        Ok(Response::Relayed(outcome)) => outcome,
        // A daemon from before relays carried through other computers.
        Ok(_) | Err(control::Error::NoAnswer | control::Error::Frame(_)) => {
            failed("the remote control daemon on this host is older than Sheprd; update it") // i18n: data
        }
        Err(control::Error::Connect(_)) => {
            failed("the remote control daemon on this host is not running") // i18n: data
        }
        Err(error) => Outcome::Failed {
            reason: error.to_string(),
        },
    }
}
