use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("could not {operation} {}", path.display())]
    File {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the SSH key is not an ssh-ed25519 public key")]
    InvalidKey,
    #[error("{} is a link, likely managed elsewhere; Sheprd will not replace it", path.display())]
    Linked { path: PathBuf },
    #[error("this computer has no home folder for authorized_keys")]
    NoHome,
    #[error("Remote Login (the SSH server) is off on this computer; turn it on and pair again")]
    ServerOff,
    #[error("{} is larger than Sheprd will rewrite", path.display())]
    TooLarge { path: PathBuf },
    #[error("this computer's user name is unknown")]
    UnknownUser,
    #[error("this computer cannot let a desktop in over SSH")]
    Unsupported,
    // keep-sorted end
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
