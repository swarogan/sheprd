//! `sheprd-relay-agent`: lets Sheprd reach this machine's SSH server
//! through relays when it has no direct path (`docs/relay-plan.md`,
//! Clients, Sheprd and SSH machines).
//!
//! The agent listens on each relay it is given with its own route key,
//! names the dial keys of the desktops in its `authorized` file, and admits
//! only their tunnel keys. Each admitted session is spliced, inside the
//! end-to-end tunnel, to the local SSH server; SSH's own encryption and
//! host keys run inside it unchanged. It runs no command and opens no other
//! port, and its logs name no address, route, or key.
//!
//! - [`cli`]: the command line, parsed from OS strings.
//! - [`files`]: the state directory and its private files.
//! - [`keys`]: the route key and tunnel keys, made once.
//! - [`authorized`]: the desktops allowed to dial.
//! - [`status`]: what `status --json` reports.
//! - [`agent`]: listening on every relay and forwarding sessions.

pub mod agent;
pub mod authorized;
pub mod cli;
pub mod error;
pub mod files;
pub mod keys;
mod listen;
mod serve;
pub mod status;

pub use error::{Error, Result};

#[cfg(test)]
mod tests;
