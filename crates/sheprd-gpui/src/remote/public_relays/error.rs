//! Why Sheprd's relay list offers nothing.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("Sheprd's relay list could not be downloaded: {0}")]
    Download(#[source] ureq::Error),
    #[error("Sheprd's relay list is not published yet.")]
    Missing,
    #[error("Sheprd's relay list was refused: {0}")]
    Refused(#[from] sheprd_relay::directory::Error),
    // keep-sorted end
}
