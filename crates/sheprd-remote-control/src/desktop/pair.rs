//! `desktop-pair`: adds a nearby Sheprd, with Sheprd carrying Bluetooth
//! over stdio (`crate::control::desktop`). The code exchange's frames go
//! out and come back as lines; the claim goes through the other computer's
//! relays; the machine is saved once its user allows it.

use super::{
    Error, Keys, Result,
    claim::{Claimant, claim},
    exchange::Exchange,
    keys::ssh_public_key,
};
use crate::control::desktop::{MAX_LINE, PairInput, PairOutput, frame_bytes};
use sheprd_client::relay_host::host_name;
use std::{
    io::{self, BufRead as _, Read as _, Write as _},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::mpsc;

/// How long the other computer has to answer each Bluetooth step.
const STEP: Duration = Duration::from_secs(30);

/// Writes one line to Sheprd.
pub(crate) fn emit(output: &PairOutput) {
    let Ok(mut line) = serde_json::to_vec(output) else {
        return;
    };
    line.push(b'\n');
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(&line).and_then(|()| stdout.flush());
}

/// Runs the pairing, reporting each step on stdout; the error is what the
/// person reads.
pub(crate) async fn pair(state_dir: PathBuf, name: String) -> Result<()> {
    let name = crate::auth::device_name(&name)?;
    let mut frames = frames();
    let state = state_dir.clone();
    let (keys, ssh_key) = tokio::task::spawn_blocking(move || {
        Ok::<_, Error>((Keys::load_or_create(&state)?, ssh_public_key(&state)?))
    })
    .await
    .map_err(|_| Error::Cancelled)??;

    let (mut exchange, start) = Exchange::start()?;
    emit(&PairOutput::frame(&start));
    let reveal = exchange.reveal(&next(&mut frames).await?)?;
    emit(&PairOutput::frame(&reveal));
    let opened = exchange.open(&next(&mut frames).await?)?;
    emit(&PairOutput::Code {
        code: opened.code.clone(),
        computer: opened.computer.clone(),
    });

    let claimant = Claimant {
        keys: &keys,
        name: &name,
        ssh_key: &ssh_key,
    };
    // Sheprd closing this child's input cancels, whatever step it is on.
    let allowed = tokio::select! {
        allowed = claim(&opened, &claimant, || emit(&PairOutput::Claimed)) => allowed?,
        () = closed(&mut frames) => return Err(Error::Cancelled),
    };
    let state = state_dir.clone();
    let record = allowed.record.clone();
    let host_keys = allowed.login.host_keys.clone();
    let id = tokio::task::spawn_blocking(move || record.save(&state, &host_keys))
        .await
        .map_err(|_| Error::Cancelled)??;
    emit(&PairOutput::Paired {
        target: format!("{}@{}", allowed.login.user, host_name(&id)),
        label: allowed.record.computer,
    });
    Ok(())
}

/// Frames Sheprd hands back, read on a thread of their own; the channel
/// closes with Sheprd's end of the pipe.
fn frames() -> mpsc::Receiver<Vec<u8>> {
    let (send, receive) = mpsc::channel(16);
    std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let mut line = String::new();
        loop {
            line.clear();
            match (&mut stdin).take(MAX_LINE as u64 + 1).read_line(&mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) if line.len() > MAX_LINE => return,
                Ok(_) => {}
            }
            let Ok(PairInput::Frame { data }) = serde_json::from_str(line.trim_end()) else {
                continue;
            };
            let Some(frame) = frame_bytes(&data) else {
                continue;
            };
            if send.blocking_send(frame).is_err() {
                return;
            }
        }
    });
    receive
}

async fn next(frames: &mut mpsc::Receiver<Vec<u8>>) -> Result<Vec<u8>> {
    tokio::time::timeout(STEP, frames.recv())
        .await
        .map_err(|_| Error::Expired)?
        .ok_or(Error::Cancelled)
}

/// Waits for Sheprd to close the input, dropping any frame meanwhile.
async fn closed(frames: &mut mpsc::Receiver<Vec<u8>>) {
    while frames.recv().await.is_some() {}
}

/// The state directory a `desktop-pair` or `relay-connect` uses.
pub(crate) fn state_dir(given: Option<&Path>) -> Option<PathBuf> {
    given
        .map(Path::to_owned)
        .or_else(crate::app::enroll::default_state_dir)
}
