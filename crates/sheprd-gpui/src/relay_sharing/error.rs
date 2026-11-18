//! Why relays could not be shared with a machine.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("The host's answer is not one this app can read")]
    Answer(#[source] serde_json::Error),
    #[error("Could not run sheprd-remote-control on the host")]
    Run(#[source] sheprd_client::Error),
    #[error("Relays cannot be shared with this kind of device")]
    Unsupported,
    // keep-sorted end
}
