//! The `[remote]` table: remote control from a phone, served by the
//! `herdr-remote-control` daemon that Sheprd installs as a user service.
//!
//! ```toml
//! [remote]
//! enabled = true
//! port = 7443
//! bind = "all"                 # or one interface address, "192.168.1.20"
//! certificate = "own"          # or { cert = "/path/cert.pem", key = "/path/key.pem" }
//! keep_running = true          # the daemon outlives Sheprd
//! notifications = "generic"    # or "excerpt": a short excerpt in each push
//! live_activities = "detailed" # or "generic": no chat names or steps
//! device_idle_days = 30         # 0 never forgets a device
//! presence = false             # hold phone alerts while its owner is at this Mac
//! relays = ["https://relay.example.org"]  # reach this computer from any network
//! github_hosts = ["ghe.example.com"]       # Enterprise Server hosts besides gh's
//! ```
//!
//! A relay may also carry a label, so lists tell relays apart:
//!
//! ```toml
//! [remote]
//! relays = [
//!   "https://relay.example.org",
//!   { endpoint = "https://fra3.example.org", name = "fra3",
//!     description = "not for public use", tags = ["eu", "private"] },
//! ]
//! ```
//!
//! Secrets and device keys never go here: the daemon keeps them in its own
//! private state directory.

use crate::app::Error;
use crate::relay_label::RelayLabel;
use serde::{Deserialize, Serialize};
use std::{fmt, net::IpAddr, ops::RangeInclusive, path::PathBuf};

pub const DEFAULT_PORT: u16 = 7443;
const DEFAULT_IDLE_DAYS: u16 = 30;
/// How long a paired device may stay away before the daemon forgets it;
/// 0 keeps it until it is revoked.
pub const IDLE_DAYS: RangeInclusive<u16> = 0..=365;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RemoteConfig {
    // keep-sorted start
    /// Where HTTPS listens: every interface, or one address.
    pub bind: Bind,
    pub certificate: Certificate,
    pub device_idle_days: u16,
    /// Whether the daemon is installed as a user service and runs.
    pub enabled: bool,
    /// Permit paired phones to read PRs with this host's GitHub CLI sign-in.
    pub github_cli: bool,
    /// GitHub Enterprise Server hosts whose remotes phones are told are on
    /// GitHub, besides github.com, `*.ghe.com`, and the hosts `gh` is signed
    /// in to; lowercase host names, without a scheme or port.
    pub github_hosts: Vec<String>,
    /// Whether the daemon keeps serving phones after Sheprd quits.
    pub keep_running: bool,
    pub live_activities: ActivityDetail,
    pub notifications: Privacy,
    pub port: u16,
    /// Whether paired iPhones near this Mac over Bluetooth hold their
    /// alerts while the desktop is in use (`herdr-nearby`, macOS only).
    pub presence: bool,
    /// Relays the daemon listens on, so paired phones reach this computer
    /// from any network (`docs/relay-plan.md`); none by default.
    pub relays: Vec<Relay>,
    // keep-sorted end
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            // keep-sorted start
            bind: Bind::default(),
            certificate: Certificate::default(),
            device_idle_days: DEFAULT_IDLE_DAYS,
            enabled: false,
            github_cli: false,
            github_hosts: Vec::new(),
            keep_running: true,
            live_activities: ActivityDetail::default(),
            notifications: Privacy::default(),
            port: DEFAULT_PORT,
            presence: false,
            relays: Vec::new(),
            // keep-sorted end
        }
    }
}

impl RemoteConfig {
    /// Refuses values the daemon could not start with.
    pub fn check(&self) -> Result<(), Error> {
        if self.port == 0 {
            return Err(Error::Port);
        }
        if !IDLE_DAYS.contains(&self.device_idle_days) {
            return Err(Error::IdleDays);
        }
        if let Some(host) = self
            .github_hosts
            .iter()
            .find(|host| !crate::control::github::valid_host(host))
        {
            return Err(Error::GitHubHost { host: host.clone() });
        }
        for relay in &self.relays {
            sheprd_relay::Endpoint::parse(&relay.endpoint).map_err(|source| Error::Relay {
                relay: relay.endpoint.clone(),
                source,
            })?;
            relay.label.check().map_err(|source| Error::RelayLabel {
                relay: relay.endpoint.clone(),
                source,
            })?;
        }
        if let Certificate::Files { cert, key } = &self.certificate {
            for (which, path) in [("cert", cert), ("key", key)] {
                if !path.is_absolute() {
                    return Err(Error::CertificatePath { which });
                }
            }
        }
        Ok(())
    }
}

/// One relay in `[remote] relays`: its address, and what the user calls it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relay {
    pub endpoint: String,
    pub label: RelayLabel,
}

impl From<String> for Relay {
    fn from(endpoint: String) -> Self {
        Self {
            endpoint,
            label: RelayLabel::default(),
        }
    }
}

impl From<&str> for Relay {
    fn from(endpoint: &str) -> Self {
        endpoint.to_owned().into()
    }
}

impl<'de> Deserialize<'de> for Relay {
    /// `"ENDPOINT"`, or `{ endpoint = "ENDPOINT", name, description, tags }`.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Labeled {
            endpoint: String,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            tags: Vec<String>,
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Relay;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a relay address, or { endpoint, name, description, tags }")
            }

            fn visit_str<E: serde::de::Error>(self, endpoint: &str) -> Result<Relay, E> {
                Ok(endpoint.into())
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Relay, A::Error> {
                let Labeled {
                    endpoint,
                    name,
                    description,
                    tags,
                } = Labeled::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Relay {
                    endpoint,
                    label: RelayLabel {
                        description,
                        name,
                        tags,
                    },
                })
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// Where the daemon's HTTPS server listens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bind {
    /// Every interface: the LAN, and Tailscale when it runs.
    #[default]
    All,
    /// One interface's address.
    Address(IpAddr),
}

impl fmt::Display for Bind {
    /// As the config file and the daemon's `--bind` write it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Address(address) => address.fmt(f),
        }
    }
}

impl<'de> Deserialize<'de> for Bind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text == "all" {
            return Ok(Self::All);
        }
        text.parse().map(Self::Address).map_err(|_| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&text),
                &"\"all\" or an IP address",
            )
        })
    }
}

/// The certificate HTTPS presents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Certificate {
    /// The daemon's own certificate authority, trusted once on the phone.
    #[default]
    Own,
    /// A certificate and key the user has, such as for their own domain.
    Files { cert: PathBuf, key: PathBuf },
}

impl<'de> Deserialize<'de> for Certificate {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Files {
            cert: PathBuf,
            key: PathBuf,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Name(String),
            Files(Files),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Name(name) if name == "own" => Ok(Self::Own),
            Raw::Name(name) => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&name),
                &"\"own\" or { cert = PATH, key = PATH }",
            )),
            Raw::Files(Files { cert, key }) => Ok(Self::Files { cert, key }),
        }
    }
}

/// What a push notification says about the chat it is for.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Privacy {
    /// "Claude needs an answer in herdr-gpui · main", without message text.
    Generic,
    /// Also a short excerpt of the message, and on a Live Activity the
    /// question with its answer buttons.
    #[default]
    Excerpt,
}

impl Privacy {
    /// As the config file and the daemon's `--privacy` write it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Excerpt => "excerpt",
        }
    }
}

/// How much a Live Activity says about its chat. A computer and each phone
/// choose for themselves, and the more private choice wins.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActivityDetail {
    /// The chat's name and current step; [`Privacy`] decides about the rest.
    #[default]
    Detailed,
    /// The agent and what it is doing in general, nothing about the chat.
    Generic,
}

impl ActivityDetail {
    /// As the config file and the daemon's `--live-activities` write it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Detailed => "detailed",
            Self::Generic => "generic",
        }
    }

    pub const fn is_detailed(&self) -> bool {
        matches!(self, Self::Detailed)
    }

    /// The more private of two choices.
    pub const fn stricter(self, other: Self) -> Self {
        match (self, other) {
            (Self::Detailed, Self::Detailed) => Self::Detailed,
            _ => Self::Generic,
        }
    }
}

/// One change to the `[remote]` table, from Settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Bind(Bind),
    Certificate(Certificate),
    DeviceIdleDays(u16),
    Enabled(bool),
    KeepRunning(bool),
    LiveActivities(ActivityDetail),
    Notifications(Privacy),
    Port(u16),
    Presence(bool),
    /// The whole list, as Settings shows it after adding or removing one;
    /// each keeps its label.
    Relays(Vec<Relay>),
}

impl Edit {
    /// The key and value the edit writes, or why it may not.
    pub fn entry(self) -> Result<(&'static str, toml_edit::Value), Error> {
        Ok(match self {
            Self::Bind(bind) => ("bind", bind.to_string().into()),
            Self::Certificate(Certificate::Own) => ("certificate", "own".into()),
            Self::Certificate(Certificate::Files { cert, key }) => {
                let text = |which, path: PathBuf| {
                    path.is_absolute()
                        .then(|| path.to_str().map(str::to_owned))
                        .flatten()
                        .ok_or(Error::CertificatePath { which })
                };
                let mut table = toml_edit::InlineTable::new();
                table.insert("cert", text("cert", cert)?.into());
                table.insert("key", text("key", key)?.into());
                ("certificate", toml_edit::Value::InlineTable(table))
            }
            Self::DeviceIdleDays(days) => {
                if !IDLE_DAYS.contains(&days) {
                    return Err(Error::IdleDays);
                }
                ("device_idle_days", i64::from(days).into())
            }
            Self::Enabled(enabled) => ("enabled", enabled.into()),
            Self::KeepRunning(keep) => ("keep_running", keep.into()),
            Self::LiveActivities(detail) => ("live_activities", detail.name().into()),
            Self::Notifications(privacy) => ("notifications", privacy.name().into()),
            Self::Port(0) => return Err(Error::Port),
            Self::Port(port) => ("port", i64::from(port).into()),
            Self::Presence(on) => ("presence", on.into()),
            Self::Relays(relays) => {
                // Written as the daemon names them, each once, so the
                // list matches the relays its status reports.
                // A relay named twice keeps its first place and the label
                // the daemon goes by: the last one given.
                let mut merged: Vec<(String, RelayLabel)> = Vec::new();
                for Relay { endpoint, label } in relays {
                    let address = relay_address(&endpoint)?;
                    label.check().map_err(|source| Error::RelayLabel {
                        relay: address.clone(),
                        source,
                    })?;
                    match merged.iter_mut().find(|(listed, _)| *listed == address) {
                        Some((_, kept)) if !label.is_empty() => *kept = label,
                        Some(_) => {}
                        None => merged.push((address, label)),
                    }
                }
                let listed = merged
                    .into_iter()
                    .map(|(address, label)| relay_value(&address, label))
                    .collect();
                ("relays", toml_edit::Value::Array(listed))
            }
        })
    }
}

/// A relay as the config file writes it: its address alone, or a table
/// with its label.
fn relay_value(address: &str, label: RelayLabel) -> toml_edit::Value {
    if label.is_empty() {
        return address.into();
    }
    let mut table = toml_edit::InlineTable::new();
    table.insert("endpoint", address.into());
    if let Some(name) = label.name {
        table.insert("name", name.into());
    }
    if let Some(description) = label.description {
        table.insert("description", description.into());
    }
    if !label.tags.is_empty() {
        table.insert(
            "tags",
            label.tags.iter().collect::<toml_edit::Array>().into(),
        );
    }
    toml_edit::Value::InlineTable(table)
}

/// A relay's address as the daemon names it in its status and saves it:
/// what was typed, trimmed, without the trailing slash of an empty path.
/// Refuses what is not a relay address, as [`RemoteConfig::check`] does.
pub fn relay_address(text: &str) -> Result<String, Error> {
    let text = text.trim();
    sheprd_relay::Endpoint::parse(text)
        .map(|endpoint| endpoint.to_string())
        .map_err(|source| Error::Relay {
            relay: text.to_owned(),
            source,
        })
}
