use std::path::PathBuf;

/// Why adding a nearby Sheprd, or reaching one through a relay, failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("the connection was cancelled")]
    Cancelled,
    #[error("the other computer did not take the claim: {0}")]
    Claim(String),
    #[error("the other computer's answer is not what a claim answers")]
    ClaimFormat,
    #[error("the codes did not match; pair again")]
    CodeMismatch,
    #[error("the other computer's user did not allow it")]
    Denied,
    #[error("nobody answered on the other computer in time")]
    Expired,
    #[error("could not {operation} {}", path.display())]
    File {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the request to the other computer failed")]
    Http(#[from] hyper::Error),
    #[error("the request to the other computer could not be written")]
    HttpRequest(#[from] hyper::http::Error),
    #[error("the code exchange did not check out; pair again")]
    Nearby(#[from] crate::auth::Error),
    #[error("the other computer's answer over Bluetooth is not one Sheprd reads")]
    NearbyFormat,
    #[error("the other computer answered over Bluetooth: {0}")]
    NearbyRefused(String),
    #[error("the other computer answered with no way to log in over SSH")]
    NoLogin,
    #[error("the other computer listens on no relays; turn on Remote control with relays there")]
    NoRelays,
    #[error("{0} is not a machine this computer added")]
    Record(String),
    #[error("another machine is already saved under {0}; remove it before pairing this one")]
    RecordConflict(String),
    #[error("the relay record is not valid")]
    RecordFormat,
    #[error("no relay of the other computer could be reached")]
    Relay(#[source] sheprd_relay::client::Error),
    #[error("the other computer's relay details are not valid")]
    RelayInfo,
    #[error("could not start the async runtime")]
    Runtime(#[source] std::io::Error),
    #[error("the connection through the relay ended")]
    Splice(#[source] std::io::Error),
    #[error("ssh-keygen could not make this computer's SSH key")]
    SshKeygen(#[source] Option<std::io::Error>),
    #[error("the TLS connection to the other computer failed")]
    Tls(#[source] std::io::Error),
    #[error("the end-to-end tunnel to the other computer failed")]
    Tunnel(#[from] sheprd_tunnel::Error),
    // keep-sorted end
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
