use std::io;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why the firewall check gave no answer. The caller logs it and warns of
/// nothing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("Could not read what {program} printed")]
    Read {
        program: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("Could not run {program}")]
    Spawn {
        program: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("{program} did not finish in time")]
    Timeout { program: &'static str },
    // keep-sorted end
}
