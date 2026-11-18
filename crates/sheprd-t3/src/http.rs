//! T3 Code's HTTP side: what a server is, signing in, and a ticket for the
//! WebSocket.
//!
//! Signing in exchanges a one-time pairing code (`t3 auth pairing create`
//! on the server's machine) for a Bearer access token, as RFC 8693 token
//! exchange. Each WebSocket then needs a short-lived ticket bought with that
//! token, since a browser socket cannot carry an `Authorization` header.
//! Neither the code, the token, nor the ticket is ever logged.

use std::{io::Read as _, time::Duration};

use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, de::DeserializeOwned};
use url::Url;

use crate::{Error, Result};

/// The orchestration protocol this client speaks.
pub const PROTOCOL: u32 = 2;

/// The scopes this client asks for: reading threads and acting in them.
pub const SCOPES: &str = "orchestration:read orchestration:operate";

/// The most an HTTP answer may weigh.
const LIMIT: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(15);

/// What a server says about itself, unauthenticated.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub environment_id: String,
    pub label: String,
    pub server_version: String,
    pub orchestration_protocol_version: u32,
}

/// A signed-in client's access.
#[derive(Debug)]
pub struct Access {
    /// Seconds the token lasts from when it was issued.
    pub expires_in: u64,
    pub scope: String,
    pub token: SecretString,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: SecretString,
    #[serde(default)]
    expires_in: f64,
    #[serde(default)]
    scope: String,
    token_type: String,
}

#[derive(Deserialize)]
struct TicketResponse {
    ticket: SecretString,
}

/// A T3 Code server, by its HTTP origin.
#[derive(Clone, Debug)]
pub struct Server {
    agent: ureq::Agent,
    origin: Url,
}

impl Server {
    pub fn new(origin: Url) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent, origin }
    }

    pub fn origin(&self) -> &Url {
        &self.origin
    }

    /// What the server is, refusing one that speaks another protocol.
    pub fn describe(&self) -> Result<Descriptor> {
        const PATH: &str = "/.well-known/t3/environment";
        let response = self.agent.get(self.url(PATH)?.as_str()).call()?;
        let descriptor: Descriptor = read(PATH, response)?;
        if descriptor.orchestration_protocol_version != PROTOCOL {
            return Err(Error::Protocol {
                expected: PROTOCOL,
                found: descriptor.orchestration_protocol_version,
            });
        }
        Ok(descriptor)
    }

    /// Exchanges the one-time pairing `code` for an access token, named on
    /// the server as `label`.
    pub fn sign_in(&self, code: &SecretString, label: &str) -> Result<Access> {
        const PATH: &str = "/oauth/token";
        let fields = [
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("subject_token", code.expose_secret()),
            (
                "subject_token_type",
                "urn:t3:params:oauth:token-type:environment-bootstrap",
            ),
            (
                "requested_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("scope", SCOPES),
            ("client_label", label),
            ("client_device_type", "desktop"),
        ];
        let response = self
            .agent
            .post(self.url(PATH)?.as_str())
            .send_form(fields)?;
        let token: TokenResponse = read(PATH, response)?;
        // Only a plain Bearer token is supported: no proof key is held.
        if !token.token_type.eq_ignore_ascii_case("bearer") {
            return Err(Error::TokenType(token.token_type));
        }
        Ok(Access {
            expires_in: token.expires_in.max(0.0) as u64,
            scope: token.scope,
            token: token.access_token,
        })
    }

    /// A short-lived ticket to open the WebSocket with.
    pub fn ticket(&self, token: &SecretString) -> Result<SecretString> {
        const PATH: &str = "/api/auth/websocket-ticket/rpc";
        let response = self
            .agent
            .post(self.url(PATH)?.as_str())
            .header("Authorization", format!("Bearer {}", token.expose_secret()))
            .send_empty()?;
        let ticket: TicketResponse = read(PATH, response)?;
        Ok(ticket.ticket)
    }

    /// The WebSocket address for `ticket`.
    pub fn socket(&self, ticket: &SecretString) -> Result<Url> {
        let mut url = self.url("/ws")?;
        let scheme = match url.scheme() {
            "https" => "wss",
            _ => "ws",
        };
        // Both are valid schemes for a URL with a host, so this cannot fail.
        let _ = url.set_scheme(scheme);
        url.query_pairs_mut()
            .append_pair("orchestrationProtocol", &PROTOCOL.to_string())
            .append_pair("wsTicket", ticket.expose_secret());
        Ok(url)
    }

    fn url(&self, path: &str) -> Result<Url> {
        Ok(self.origin.join(path)?)
    }
}

/// A JSON answer of at most [`LIMIT`] bytes, or the status that refused it.
fn read<T: DeserializeOwned>(
    path: &'static str,
    mut response: ureq::http::Response<ureq::Body>,
) -> Result<T> {
    let status = response.status().as_u16();
    if !(200..=299).contains(&status) {
        return Err(Error::Status { path, status });
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Http(Box::new(ureq::Error::Io(source))))?;
    if bytes.len() as u64 > LIMIT {
        return Err(Error::TooLarge {
            limit: LIMIT as usize,
        });
    }
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
