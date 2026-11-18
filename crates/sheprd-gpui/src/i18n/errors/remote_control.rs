//! Remote control's failures in the app, from `error-remote-control.ftl`:
//! its settings and service install (`app::Error`) and the daemon's
//! control socket (`control::Error`), and enrolling a host's daemon
//! (`enroll::Error`). Program names, paths, exit statuses,
//! and the daemon's and commands' own words are placeables.

use super::Localize;
use crate::i18n::fl;
use sheprd_remote_control::{app, app::enroll, control, relay_label};

impl Localize for app::Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::CertificatePath { which } => {
                fl!("error-remote-control-certificate-path", key = *which)
            }
            Self::Control(error) => error.localize(),
            Self::Enroll(error) => error.localize(),
            Self::GitHubHost { host } => {
                fl!("error-remote-control-github-host", host = host.as_str())
            }
            Self::IdleDays => fl!("error-remote-control-idle-days"),
            Self::Io {
                operation, path, ..
            } => fl!(
                "error-remote-control-io",
                operation = *operation,
                path = path.display().to_string()
            ),
            Self::NoStateDir => fl!("error-remote-control-no-state-dir"),
            Self::PairingAddress => fl!("error-remote-control-pairing-address"),
            Self::PairingUrl(_) => fl!("error-remote-control-pairing-url"),
            Self::PathText { path } => fl!(
                "error-remote-control-path-text",
                path = path.display().to_string()
            ),
            Self::Port => fl!("error-remote-control-port"),
            Self::ProgramMissing => fl!("error-remote-control-program-missing"),
            Self::Qr(_) => fl!("error-remote-control-qr"),
            Self::Relay { relay, .. } => {
                fl!("error-remote-control-relay", relay = relay.as_str())
            }
            Self::RelayLabel { relay, source } => fl!(
                "error-remote-control-relay-label",
                relay = relay.as_str(),
                reason = source.localize()
            ),
            Self::Service {
                program,
                status,
                stderr,
            } => fl!(
                "error-remote-control-service",
                program = *program,
                status = status.to_string(),
                stderr = stderr.as_str()
            ),
            Self::Spawn { program, .. } => fl!("error-remote-control-spawn", program = *program),
            Self::Unexpected => fl!("error-remote-control-unexpected"),
            Self::UnknownUser => fl!("error-remote-control-unknown-user"),
            Self::Unsupported => fl!("error-remote-control-unsupported"),
            // keep-sorted end
        }
    }
}

impl Localize for relay_label::Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::Blank { field } => fl!("error-remote-control-relay-label-blank", field = *field),
            Self::Control { field } => {
                fl!("error-remote-control-relay-label-control", field = *field)
            }
            Self::Length { field, max } => fl!(
                "error-remote-control-relay-label-length",
                field = *field,
                max = max.to_string()
            ),
            Self::Tags => fl!(
                "error-remote-control-relay-label-tags",
                max = relay_label::TAGS_MAX.to_string()
            ),
            // keep-sorted end
        }
    }
}

impl Localize for control::Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::Connect(_) => fl!("error-remote-control-connect"),
            Self::Daemon { message, .. } => {
                fl!("error-remote-control-daemon", message = message.as_str())
            }
            Self::Frame(error) => error.localize(),
            Self::NoAnswer => fl!("error-remote-control-no-answer"),
            // keep-sorted end
        }
    }
}

impl Localize for enroll::Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::ApnsSize => fl!("error-remote-control-enroll-apns-size"),
            Self::Config { path, .. } => fl!(
                "error-remote-control-enroll-config",
                path = path.display().to_string()
            ),
            Self::NoHome => fl!("error-remote-control-enroll-no-home"),
            Self::NotStarted(_) => fl!("error-remote-control-enroll-not-started"),
            Self::Outdated => fl!("error-remote-control-enroll-outdated"),
            Self::SettingsLocked { path } => fl!(
                "error-remote-control-enroll-settings-locked",
                path = path.display().to_string()
            ),
            Self::Unexpected => fl!("error-remote-control-enroll-unexpected"),
            // keep-sorted end
        }
    }
}
