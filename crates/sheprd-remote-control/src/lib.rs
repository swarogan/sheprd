//! Remote control: a daemon that serves this machine's `herdr-acp` chats to
//! paired phones over HTTPS, the control protocol Sheprd uses to run it,
//! and Sheprd's side without the server ([`app`]).
#![doc = include_str!("../README.md")]

#[cfg(feature = "server")]
mod ansi;
#[cfg(feature = "server")]
mod apns;
pub mod app;
#[cfg(feature = "server")]
mod auth;
pub mod control;
pub mod core_target;
#[cfg(feature = "server")]
pub mod desktop;
mod error;
#[cfg(feature = "server")]
mod hub;
#[cfg(feature = "server")]
mod load;
#[cfg(feature = "server")]
mod native;
#[cfg(feature = "server")]
mod push;
#[cfg(feature = "server")]
mod relay;
pub mod relay_label;
#[cfg(feature = "server")]
pub mod server;
#[cfg(feature = "server")]
mod ssh_access;
#[cfg(feature = "server")]
mod tls;
#[cfg(feature = "server")]
mod web;

pub use error::{Error, Result};
/// `sheprd-remote-control relays --offer`: relays another computer shares.
#[cfg(feature = "server")]
pub use relay::shared as shared_relays;
