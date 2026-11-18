/// Why listening on a relay, or serving a session through one, failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("could not keep the relay keys")]
    Auth(#[from] crate::auth::Error),
    #[error("the relay could not be reached")]
    Client(#[from] sheprd_relay::client::Error),
    #[error("a desktop's SSH session failed")]
    Ssh(#[source] std::io::Error),
    #[error("a stored relay key is not one")]
    StoredKey,
    #[error("the end-to-end tunnel failed")]
    Tunnel(#[from] sheprd_tunnel::Error),
    // keep-sorted end
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
