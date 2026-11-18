//! Offering relays to another machine's daemon over SSH, as `device_tags`
//! offers tags. Everything here blocks: run it on the background executor.

use super::Error;
use crate::teleport::Host;
use serde::Deserialize;
use sheprd_client::{ConnectTarget, ScriptLimits};
use sheprd_remote_control::control::relays::Offer;
use std::{sync::atomic::AtomicBool, time::Duration};

/// What `relays` may print: one small JSON answer.
const OUTPUT: u64 = 16 * 1024;
const IDLE: Duration = Duration::from_secs(20);

/// Finds the program without installing it: a machine without one has no
/// daemon for a phone to reach, through a relay or otherwise. A build older
/// than shared relays refuses the subcommand and prints nothing, as a
/// machine without the program does. Standard input is drained so the
/// sender is never left writing to no one.
pub(crate) const SCRIPT: &str = "\
program=$(command -v sheprd-remote-control || command -v herdr-remote-control || true)
[ -n \"$program\" ] || program=\"$HOME/.local/bin/sheprd-remote-control\"
if [ -x \"$program\" ]; then \"$program\" relays --offer 2>/dev/null || true; fi
cat >/dev/null
";

/// Offers `offer` to the machine's daemon. Returns whether it took them;
/// `false` when the machine has nothing that keeps relays.
pub(super) fn share(
    target: &ConnectTarget,
    offer: Offer,
    cancelled: &AtomicBool,
) -> Result<bool, Error> {
    let host = Host::new(target).map_err(|_| Error::Unsupported)?;
    let input = serde_json::to_vec(&offer).map_err(Error::Answer)?;
    let mut output = Vec::new();
    host.stream(
        SCRIPT,
        input.as_slice(),
        &mut output,
        ScriptLimits {
            output: OUTPUT,
            idle: IDLE,
        },
        cancelled,
    )
    .map_err(Error::Run)?;
    parse(&output)
}

/// Whether the host's answer says it took the relays; nothing printed
/// means it could not.
pub(crate) fn parse(output: &[u8]) -> Result<bool, Error> {
    #[derive(Deserialize)]
    struct Answer {
        #[allow(dead_code)]
        relays: Vec<String>,
    }
    if output.iter().all(u8::is_ascii_whitespace) {
        return Ok(false);
    }
    serde_json::from_slice::<Answer>(output).map_err(Error::Answer)?;
    Ok(true)
}
