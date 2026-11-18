//! Where a phone typed a prompt, for the chat program to tell the agent:
//! Sheprd's app or its web app, and the way the phone reached this
//! computer. The daemon decides it from the connection; whatever a phone
//! claims in its command is replaced.

use std::{convert::Infallible, net::IpAddr};

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts},
};
use sheprd_chat_protocol::{Device, Origin, Via};

use super::Peer;

/// Marks a request that came through Sheprd's relay rather than over the
/// network.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Relayed;

/// The origin of the prompts a request carries.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TypedFrom(pub(crate) Origin);

impl<S: Send + Sync> FromRequestParts<S> for TypedFrom {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Infallible> {
        let header = |name| {
            parts
                .headers
                .get(name)
                .and_then(|value: &header::HeaderValue| value.to_str().ok())
        };
        let peer = parts
            .extensions
            .get::<ConnectInfo<Peer>>()
            .map(|ConnectInfo(Peer(peer))| peer.ip());
        let via = via(
            parts.extensions.get::<Relayed>().is_some(),
            peer,
            header(header::HOST),
        );
        Ok(Self(Origin::new(device(header(header::USER_AGENT)), via)))
    }
}

/// Browsers name themselves `Mozilla/…`; Sheprd's app sends the system's
/// default, which starts with its bundle name.
pub(crate) fn device(user_agent: Option<&str>) -> Device {
    match user_agent {
        Some(agent) if agent.starts_with("Mozilla/") => Device::PhoneWeb,
        _ => Device::PhoneApp,
    }
}

/// The way a request reached this computer: the relay when it says so,
/// else what the phone's address or the name it dialed shows.
pub(crate) fn via(relayed: bool, peer: Option<IpAddr>, host: Option<&str>) -> Via {
    if relayed {
        return Via::Relay;
    }
    let host = host.map(|host| {
        host.rsplit_once(':')
            .filter(|(_, port)| port.bytes().all(|byte| byte.is_ascii_digit()))
            .map_or(host, |(name, _)| name)
    });
    if host.is_some_and(|host| host.ends_with(".ts.net")) {
        return Via::Tailscale;
    }
    let Some(peer) = peer.map(|peer| peer.to_canonical()) else {
        return Via::Other;
    };
    match peer {
        _ if peer.is_loopback() => Via::Local,
        // A phone reaching this listener from carrier-grade NAT space is a
        // tailnet peer: a carrier's own NAT never shows its inside address.
        IpAddr::V4(ip) if ip.octets()[0] == 100 && ip.octets()[1] & 0xc0 == 64 => Via::Tailscale,
        IpAddr::V6(ip) if ip.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0] => Via::Tailscale,
        IpAddr::V4(ip) if ip.is_private() || ip.is_link_local() => Via::Lan,
        IpAddr::V6(ip) if ip.is_unique_local() || ip.is_unicast_link_local() => Via::Lan,
        _ => Via::Other,
    }
}
