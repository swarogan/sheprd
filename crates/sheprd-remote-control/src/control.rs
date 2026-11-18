//! The wire between Sheprd and the daemon: one request and one response
//! per connection, as length-prefixed JSON frames over a private endpoint
//! (`sheprd_client::private_endpoint`) in the daemon's state directory, so
//! only this user reaches it. No Tokio: Sheprd calls [`request`] from a
//! background thread.
//!
//! Secrets never cross it except a pairing offer's URL, whose token is
//! valid for one minute and one claim. Device keys and session tokens stay
//! in the daemon.

use sheprd_chat_protocol::{read_frame, write_frame};
pub mod app_update;
pub mod changes;
pub mod desktop;
pub mod edited;
pub mod github;
pub mod host_relay;
pub mod hosts;
pub mod layout;
pub mod pull_requests;
pub mod relays;
pub mod shortcuts;
pub mod tags;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

/// How long a request waits for a pipe instance on Windows.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// How long a client waits for the daemon's answer.
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long `Request::ChangeAsks` waits for a phone's question: within
/// [`RESPONSE_TIMEOUT`], so an empty answer is not taken for a dead daemon.
pub const ASK_WAIT: Duration = Duration::from_secs(4);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("could not reach the remote control daemon")]
    Connect(#[source] std::io::Error),
    #[error("the remote control daemon refused: {message}")]
    Daemon { code: ErrorCode, message: String },
    #[error(transparent)]
    Frame(#[from] sheprd_chat_protocol::Error),
    #[error("the remote control daemon closed the connection without answering")]
    NoAnswer,
    // keep-sorted end
}

/// Where the daemon whose state lives in `state_dir` listens for Sheprd.
pub fn address(state_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        // A pipe name, unique per state directory.
        let digest = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            state_dir.to_string_lossy().as_bytes(),
        );
        return PathBuf::from(format!(r"\\.\pipe\herdr-remote-{}", digest.simple()));
    }
    state_dir.join("control.sock")
}

/// Removes what a daemon's listener left in `state_dir`.
pub fn release(state_dir: &Path) {
    sheprd_client::private_endpoint::release(&address(state_dir));
}

/// Sends one request and waits for its response. Blocking.
pub fn request(address: &Path, request: &Request) -> Result<Response, Error> {
    let connection = sheprd_client::private_endpoint::Connection::connect(address, CONNECT_TIMEOUT)
        .map_err(Error::Connect)?;
    let (mut reader, writer, closer) = connection.split().map_err(Error::Connect)?;
    reader
        .set_timeout(Some(RESPONSE_TIMEOUT))
        .map_err(Error::Connect)?;
    let mut writer = BufWriter::new(writer);
    write_frame(&mut writer, request)?;
    writer.flush().map_err(Error::Connect)?;
    let answer = read_frame::<_, Response>(&mut BufReader::new(reader));
    closer.close();
    match answer?.ok_or(Error::NoAnswer)? {
        Response::Error { code, message } => Err(Error::Daemon { code, message }),
        response => Ok(response),
    }
}

/// A pairing in progress, named by the daemon.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PairingId(pub String);

/// A paired device, named by the daemon.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "request")]
pub enum Request {
    /// Whether the daemon runs, where it listens, and who is connected.
    Status,
    /// Starts a pairing, or rotates the token of the open one: the QR the
    /// dialog shows. Each call voids the previous token.
    Pair,
    /// Where the pairing stands; the dialog polls it.
    Pairing {
        id: PairingId,
    },
    /// The user's answer to a claimed pairing in the desktop dialog, with
    /// the matching code it showed: an approval for any other claim is
    /// refused.
    Approve {
        id: PairingId,
        allow: bool,
        code: String,
        /// The hosts ticked in the dialog, for the phone to be paired
        /// with too (`hosts`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hosts: Vec<hosts::HostId>,
        /// Whether the phone may borrow this computer's GitHub sign-in
        /// (`github`), as the dialog's switch says. On unless an approval
        /// says otherwise, as an older Sheprd's never does.
        #[serde(default = "yes", skip_serializing_if = "is_yes")]
        share_github: bool,
    },
    /// Closes the dialog's pairing; its token stops working.
    CancelPairing {
        id: PairingId,
    },
    Devices,
    /// Revokes a device: its sessions end and its push subscription is
    /// dropped at once. Sheprd asks the user before sending it.
    Revoke {
        device: DeviceId,
    },
    /// Whether `device` may borrow this computer's GitHub sign-in. Turned
    /// off, the device gets no further token; one it holds keeps working
    /// until it expires, or, from `gh`, until `gh` signs out.
    #[serde(rename = "share_github")]
    ShareGitHub {
        device: DeviceId,
        share: bool,
    },
    /// Stops the daemon. The service manager does not restart it.
    Shutdown,
    /// The status marks the user picked in Sheprd, by its config name
    /// (`ui.status_icons`), for the phone's lists to draw the same. A set
    /// the phone cannot draw shows as dots.
    Indicators {
        set: String,
    },
    /// The desktop's cached PR badges, fenced by daemon boot and workspace identity.
    PullRequests {
        snapshot: pull_requests::PullRequests,
    },
    /// A phone nearby starts pairing over Bluetooth, which Sheprd relays:
    /// the first step of the code exchange (`docs/ios-app-plan.md`,
    /// "Nearby pairing, exactly"). Refused unless a pairing dialog is open.
    NearbyStart {
        /// The phone's one-time P-256 key, SEC1 uncompressed, base64url.
        public_key: String,
    },
    /// The second step: the phone's nonce, answered with the daemon's and
    /// the sealed pairing token.
    NearbyReveal {
        session: String,
        nonce: String,
    },
    /// A paired phone near over Bluetooth says hello with its daily hint,
    /// which Sheprd relays: answered with a nonce, signed by this
    /// computer's CA (`docs/ios-app-plan.md`, "Presence, exactly").
    PresenceChallenge {
        hint: String,
    },
    /// The phone's signature over `herdr-presence-v1 || CA fingerprint ||
    /// nonce`; the nonce names the device it was given for.
    PresenceProve {
        nonce: String,
        signature: String,
    },
    /// Whether a proven phone's owner is at this computer: while near, the
    /// device's alerts are held. A mark not renewed lapses after two
    /// minutes.
    PresenceSet {
        device: DeviceId,
        near: bool,
    },
    /// The SSH and cloud hosts Sheprd connects to, which a phone paired
    /// here may be paired with too. Replaces the previous list.
    Hosts {
        hosts: Vec<hosts::Host>,
    },
    /// Takes the phones' requests to be paired with hosts that Sheprd
    /// has not taken yet.
    HostPairings,
    /// How Sheprd's attempt to pair `device` with `host` ended.
    HostPaired {
        device: DeviceId,
        host: hosts::HostId,
        /// The key of the request it took (`HostPairing::public_key`), so
        /// an attempt that outlasted a retry cannot answer the retry.
        /// Absent from an older Sheprd, which matches any request.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        public_key: Option<String>,
        outcome: hosts::HostOutcome,
    },
    /// The devices revoked here that hosts still hold.
    HostRevokes,
    /// A host no longer holds the device.
    HostRevoked(hosts::HostRevoke),
    /// This computer's tags: offers Sheprd's copy, if any, and answers with
    /// the ones that stand (`tags`).
    Tags {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offer: Option<tags::ComputerTags>,
    },
    /// This computer's text shortcuts: offers Sheprd's copy, if any, and
    /// answers with the ones that stand (`shortcuts`).
    Shortcuts {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offer: Option<shortcuts::TextShortcuts>,
    },
    /// Relays Sheprd on another computer shares with this one, replacing
    /// what that computer shared before (`relays`). Answered with every
    /// relay the daemon listens on now. Sheprd never sends it to its own
    /// daemon, which follows `[remote] relays`.
    ShareRelays(relays::Offer),
    /// On a host: adds a phone another computer's user allowed, as
    /// `herdr-remote-control enroll` asks. Sheprd never sends it to its
    /// own daemon.
    Enroll(hosts::Enroll),
    /// Where the app's updater stands, sent with each poll while a window
    /// runs it. Answered with whether a phone asked to update. `instance`
    /// names this run of the app: a request is only for the run a phone saw.
    AppUpdate {
        instance: u32,
        update: app_update::AppUpdate,
    },
    /// Takes the phones' questions about workspaces, waiting up to
    /// [`ASK_WAIT`] for one. Sheprd sends it while a phone is connected.
    ChangeAsks,
    /// What Sheprd found for an [`changes::Ask`].
    ChangeAnswer {
        id: changes::AskId,
        answer: changes::Answer,
    },
    /// What Sheprd did with a [`layout::Ask`].
    LayoutAnswer {
        id: changes::AskId,
        answer: layout::Answer,
    },
    /// Takes the phones' requests to carry their relay keys to another of
    /// their computers (`host_relay`) that Sheprd has not taken yet. An
    /// older daemon refuses it, and Sheprd then carries none.
    HostRelays,
    /// What carrying `relay` came to.
    HostRelayed {
        /// The phone, as this computer knows it.
        device: DeviceId,
        /// Boxed, as the largest request, so every request stays small.
        relay: Box<host_relay::Request>,
        outcome: Box<host_relay::Outcome>,
    },
    /// On a host: keeps a phone's relay keys, signed with its own device
    /// key there, as `herdr-remote-control relay-enroll` asks.
    RelayEnroll(Box<host_relay::Request>),
    /// Takes the phones' asks for Sheprd's GitHub sign-in, waiting up to
    /// [`ASK_WAIT`] for one. Sheprd sends it while a phone is connected or
    /// its status counts asks waiting (`Status::github_asks`).
    #[serde(rename = "github_lease_asks")]
    GitHubLeaseAsks {
        /// Sheprd's main github.com account now, which phones are told
        /// this computer lends; `None` when it has none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        login: Option<String>,
    },
    /// Sheprd's answer to a [`github::LeaseAsk`].
    #[serde(rename = "github_lease")]
    GitHubLease {
        id: changes::AskId,
        answer: github::LeaseAnswer,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "response")]
pub enum Response {
    Status(Status),
    Offer(Offer),
    Pairing(PairingState),
    Devices {
        devices: Vec<Device>,
    },
    /// The request was carried out.
    Done,
    Error {
        code: ErrorCode,
        message: String,
    },
    NearbyStarted(NearbyStarted),
    NearbyRevealed(NearbyRevealed),
    /// The nonce a presence challenge asks the phone to sign, with this
    /// computer's CA certificate (DER) and its signature over
    /// `herdr-presence-challenge-v1 || nonce || device id`, all base64url.
    PresenceChallenge {
        certificate: String,
        nonce: String,
        signature: String,
    },
    /// The phone proved it is `device`. `addresses` is the computer's
    /// current origins sealed to the device's notify key, for the phone to
    /// find it again when its address changed; `None` when the device
    /// registered no key.
    PresenceProven {
        device: DeviceId,
        addresses: Option<String>,
    },
    HostPairings {
        pairings: Vec<hosts::HostPairing>,
    },
    HostRevokes {
        revokes: Vec<hosts::HostRevoke>,
    },
    Enrolled(hosts::Enrolled),
    Tags(tags::ComputerTags),
    Shortcuts(shortcuts::TextShortcuts),
    /// The relays the daemon listens on, its own and those shared with it.
    Relays {
        relays: Vec<String>,
    },
    AppUpdate {
        /// A phone asked to update since the last report. Taken by reading.
        requested: bool,
    },
    ChangeAsks {
        asks: Vec<changes::Ask>,
        /// Changes phones ask for in Herdr 2 tabs' layouts. An older
        /// daemon sends none, and an older Sheprd leaves them to time out.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        layout: Vec<layout::Ask>,
    },
    HostRelays {
        jobs: Vec<host_relay::Job>,
    },
    Relayed(host_relay::Outcome),
    #[serde(rename = "github_lease_asks")]
    GitHubLeaseAsks {
        asks: Vec<github::LeaseAsk>,
    },
}

/// The daemon's answer to a nearby phone's first step, the same JSON over
/// `POST /api/nearby/start` and the control socket.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NearbyStarted {
    pub session: String,
    /// The name the phone shows for this computer.
    pub computer: String,
    /// SHA-256 of the certificate authority, 64 uppercase hex digits.
    pub fingerprint: String,
    /// The daemon's one-time P-256 key, SEC1 uncompressed, base64url.
    pub public_key: String,
    /// SHA-256 of the domain, both keys, and the daemon's nonce, base64url:
    /// the daemon cannot change its nonce after seeing the phone's.
    pub commit: String,
}

/// The daemon's answer to the second step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NearbyRevealed {
    /// The daemon's nonce, base64url.
    pub nonce: String,
    /// AES-256-GCM nonce then ciphertext of `{"token", "origins"}`, base64url.
    pub sealed: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No such pairing, or it expired.
    UnknownPairing,
    /// No such device.
    UnknownDevice,
    /// The pairing is not waiting for an answer (not claimed yet, or done).
    NotClaimed,
    /// The request does not fit this daemon's version.
    Unsupported,
    /// Nearby pairing needs the pairing dialog open on the computer.
    NoPairing,
    /// The daemon could not write its state; nothing changed, and the
    /// same request may succeed later.
    Unsaved,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// The local Herdr session whose workspaces this daemon exposes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub boot_id: String,
    /// Interface metadata keyed by advertised origin; absent on older daemons.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub address_details: std::collections::BTreeMap<String, AddressDetails>,
    /// The daemon's version, for Sheprd to restart an older one.
    pub version: String,
    /// The port HTTPS listens on.
    pub port: u16,
    /// Every `https://` origin the certificate covers, Tailscale first, then LAN.
    pub addresses: Vec<String>,
    /// SHA-256 of the certificate authority, colon-separated hex.
    pub fingerprint: String,
    /// The name this computer gives itself, which phones show.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub computer: String,
    /// Whether the daemon reaches a Herdr daemon on this machine.
    pub herdr: bool,
    pub chats: usize,
    pub devices: usize,
    /// Devices with an open stream now.
    pub connected: usize,
    /// Phones' asks for Sheprd's GitHub sign-in waiting to be taken
    /// (`Request::GitHubLeaseAsks`); an older daemon has none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub github_asks: usize,
    /// Each relay this computer listens on, by endpoint; absent when it
    /// listens on none, or the daemon is older.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub relays: std::collections::BTreeMap<String, RelayStatus>,
}

/// One relay as `sheprd remote status` shows it: where it stands, and the
/// label `[remote] relays` gives it, whose fields an older daemon omits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RelayStatus {
    #[serde(flatten)]
    pub label: crate::relay_label::RelayLabel,
    #[serde(flatten)]
    pub state: RelayState,
}

impl From<RelayState> for RelayStatus {
    fn from(state: RelayState) -> Self {
        Self {
            label: crate::relay_label::RelayLabel::default(),
            state,
        }
    }
}

/// Where one relay stands.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RelayState {
    Connecting,
    Listening {
        /// The round trip to the relay, in milliseconds, which tells how
        /// near it is; absent until measured, or from an older daemon. An
        /// older app reads the state and ignores it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rtt_ms: Option<u32>,
    },
    /// The last attempt failed; another follows.
    Failed {
        error: String,
    },
}

/// What the pairing dialog shows: a QR of `url`, valid until `expires_in_ms`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub address_details: std::collections::BTreeMap<String, AddressDetails>,
    pub id: PairingId,
    /// `https://ADDRESS:PORT/pair#t=TOKEN&fp=FINGERPRINT`.
    pub url: String,
    pub fingerprint: String,
    pub expires_in_ms: u64,
    /// Every origin the phone may reach this computer at.
    pub addresses: Vec<String>,
}

/// Observed on the daemon's host, never inferred from the GUI host's adapters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AddressDetails {
    pub interfaces: Vec<String>,
    pub kind: AddressKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressKind {
    // keep-sorted start
    Bridge,
    Local,
    Loopback,
    Tailscale,
    // keep-sorted end
    // Serde's forward-compatible fallback must be last.
    #[default]
    #[serde(other)]
    Other,
}

impl AddressKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bridge => "Network bridge",
            Self::Local => "Local network",
            Self::Loopback => "This computer only",
            Self::Other => "Network address",
            Self::Tailscale => "Tailscale",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum PairingState {
    /// No phone has claimed the token yet.
    Waiting {
        expires_in_ms: u64,
    },
    /// A phone claimed it: the dialog shows its name and the matching code
    /// and asks the user.
    Claimed {
        device_name: String,
        code: String,
        /// The origin of the Home Screen app that claimed it, when that app
        /// was installed from another computer: approving admits it to this
        /// computer's API, so the dialog names it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
        /// Another computer's Sheprd asking to open terminals here over SSH,
        /// through this computer's relays: approving adds its key to
        /// `~/.ssh/authorized_keys`, so the dialog says so.
        #[serde(default, skip_serializing_if = "is_no")]
        ssh: bool,
    },
    Approved {
        device: DeviceId,
    },
    Denied,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    /// Milliseconds since the Unix epoch.
    pub created_ms: u64,
    pub last_seen_ms: Option<u64>,
    /// Whether it receives push notifications.
    pub push: bool,
    /// Whether it has a stream open now.
    pub connected: bool,
    /// Whether it may borrow this computer's GitHub sign-in (`github`).
    #[serde(default = "yes")]
    pub share_github: bool,
    /// The computer that vouched for it, when another computer's user
    /// allowed it (`hosts`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// The other hosts it was paired with from here, or may be.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<hosts::HostLink>,
}

/// Serde's default for a switch that is on unless said otherwise.
fn yes() -> bool {
    true
}

/// Whether a switch is on, so it is left out; serde passes a reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_yes(value: &bool) -> bool {
    *value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_no(value: &bool) -> bool {
    !*value
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
