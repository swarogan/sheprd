//! Why Sheprd's relay list offers nothing, from `error-public-relays.ftl`.

use super::Localize;
use crate::{i18n::fl, remote::public_relays::Error};

impl Localize for Error {
    fn localize(&self) -> String {
        match self {
            // The transport's and the list's own reasons are shown as they are.
            Self::Download(source) => {
                fl!("error-public-relays-download", error = source.to_string())
            }
            Self::Missing => fl!("error-public-relays-missing"),
            Self::Refused(source) => {
                fl!("error-public-relays-refused", error = source.to_string())
            }
        }
    }
}
