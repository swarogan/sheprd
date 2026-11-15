//! Sheprd's own relays, for Settings to offer: the list at
//! `sheprd.org/relays.json`, taken only when one of the signers compiled
//! into `sheprd_relay::directory::signed` vouched for its exact bytes. A
//! list nobody signed, or an expired one, offers nothing.

mod error;

pub use error::Error;
use sheprd_relay::directory::{
    self, Directory,
    signed::{self, Signatures, Signer},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long the list and its signatures may take, each.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The relays Sheprd lists, and who vouched for the list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Listed {
    /// When the list stops being taken, in Unix seconds.
    pub(crate) expires: u64,
    pub(crate) relays: Vec<Public>,
    pub(crate) signer: Signer,
}

impl Listed {
    /// Whether the list still offers its relays at `now` (Unix seconds).
    pub(crate) fn expired(&self, now: u64) -> bool {
        now >= self.expires
    }
}

/// Now, in Unix seconds.
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// One of Sheprd's relays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Public {
    /// Its address, as `[remote] relays` and the daemon's status name it.
    pub(crate) address: String,
    /// A name to show, such as `Frankfurt`.
    pub(crate) name: Option<String>,
    /// A short region code, such as `fra1`.
    pub(crate) region: Option<String>,
}

/// Downloads the list and its signatures, and checks them. Blocking: run
/// it off the UI thread.
pub(crate) fn fetch() -> Result<Listed, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(0)
        .build()
        .into();
    let bytes = download(&agent, directory::URL, directory::MAX_BYTES)?.ok_or(Error::Missing)?;
    let openpgp = download(&agent, signed::OPENPGP_URL, signed::MAX_SIGNATURE);
    let ed25519 = download(&agent, signed::ED25519_URL, signed::MAX_SIGNATURE);
    settle(&bytes, openpgp, ed25519, now())
}

/// What the list offers given how each signature's download went. One
/// signer is enough, so a signature that could not be downloaded counts as
/// absent while the other vouches for the list; only when no signature
/// that arrived does is the failed download the reason reported.
pub(crate) fn settle(
    bytes: &[u8],
    openpgp: Result<Option<Vec<u8>>, Error>,
    ed25519: Result<Option<Vec<u8>>, Error>,
    now: u64,
) -> Result<Listed, Error> {
    let (openpgp, openpgp_failed) = arrived(openpgp);
    let (ed25519, ed25519_failed) = arrived(ed25519);
    let signatures = Signatures {
        openpgp: openpgp.as_deref(),
        ed25519: ed25519.as_deref(),
    };
    match (
        listed(bytes, signatures, now),
        openpgp_failed.or(ed25519_failed),
    ) {
        (Err(Error::Refused(directory::Error::Unsigned)), Some(failed)) => Err(failed),
        (result, _) => result,
    }
}

/// A download's signature, if one arrived, and its failure, if it failed.
fn arrived(download: Result<Option<Vec<u8>>, Error>) -> (Option<Vec<u8>>, Option<Error>) {
    match download {
        Ok(signature) => (signature, None),
        Err(error) => (None, Some(error)),
    }
}

/// What a downloaded list offers at `now` (Unix seconds), once checked.
pub(crate) fn listed(bytes: &[u8], signatures: Signatures<'_>, now: u64) -> Result<Listed, Error> {
    Ok(Directory::verify(bytes, signatures, now, None)?.into())
}

impl From<signed::Signed> for Listed {
    fn from(signed: signed::Signed) -> Self {
        Self {
            expires: signed.expires,
            relays: signed
                .directory
                .relays
                .into_iter()
                .map(|relay| Public {
                    address: relay.endpoint.to_string(),
                    name: relay.name,
                    region: relay.region,
                })
                .collect(),
            signer: signed.signer,
        }
    }
}

/// The file at `url`, at most `limit` bytes of it, or `None` when the site
/// has none: a signature either signer may leave out.
fn download(agent: &ureq::Agent, url: &str, limit: usize) -> Result<Option<Vec<u8>>, Error> {
    let mut response = match agent.get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(source) => return Err(Error::Download(source)),
    };
    // One byte past the limit, so a longer file is refused, not cut short.
    let bytes = response
        .body_mut()
        .with_config()
        .limit(limit as u64 + 1)
        .read_to_vec()
        .map_err(Error::Download)?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests;
