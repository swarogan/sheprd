//! The `[code]` table: VS Code served as a tab.

use serde::Deserialize;

/// VS Code as a tab of a space's editor groups, served by
/// `code serve-web`. Each space that shows it gets
/// its own page, which starts at the server's address and then goes
/// wherever it navigates.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CodeConfig {
    /// Where the server comes from. Unset, it is `address` when `url` is
    /// set, as before there was a choice, else `start` once VS Code is found.
    pub(crate) mode: Option<CodeMode>,
    /// The server to use in `address` mode. `None` leaves the panel empty,
    /// with a hint to set it.
    pub(crate) url: Option<crate::browser::WebUrl>,
    /// The port of the server the app starts, picked once and kept: VS Code
    /// keeps its settings in the storage of the page's origin.
    pub(crate) port: Option<std::num::NonZeroU16>,
    /// The user accepted VS Code's server license in the app. Until then
    /// the app starts nothing, since it would accept it for them.
    pub(crate) license_accepted: bool,
}

/// Where VS Code tabs' server comes from.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CodeMode {
    /// The app starts `code serve-web` itself.
    Start,
    /// A server the user runs, at `url`.
    Address,
}

/// One change to the `[code]` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CodeEdit {
    /// Sets the address, or removes it with `None`.
    Url(Option<crate::browser::WebUrl>),
    Mode(CodeMode),
    Port(std::num::NonZeroU16),
    AcceptLicense,
}
