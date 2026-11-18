//! Adding a nearby Sheprd that is on another network, Sheprd's side
//! (`docs/relay-plan.md`, Adding a nearby Sheprd from another network).
//! `sheprd-remote-control desktop-pair` runs the code exchange, the claim
//! through the other computer's relays, and saves what `ssh` needs; this
//! starts it and carries its frames over Bluetooth through the [`Station`]
//! (`super::Station`). Tokio stays in that child.
//!
//! [`install`] also points `sheprd-client` at `relay-connect`, so every
//! `ssh` to a machine added this way goes through its relays.

use super::Error;
use sheprd_remote_control::control::desktop::{MAX_LINE, PairInput, PairOutput};
use std::{
    io::{self, BufRead as _, Read as _, Write as _},
    path::PathBuf,
    process::{Child, Stdio},
    sync::mpsc::{self, Receiver, SyncSender, TrySendError},
    thread,
};

/// Lines waiting in each direction.
const QUEUE: usize = 16;

/// Lets `ssh` reach machines added over Bluetooth, through
/// `sheprd-remote-control relay-connect`, once it is found beside the app.
pub(crate) fn install(cx: &mut gpui::App) {
    cx.background_executor()
        .spawn(async {
            let program = crate::bundled::locate(crate::remote::service::PROGRAM);
            let state_dir = crate::remote::state_dir().ok();
            let proxy = program.zip(state_dir).map(|(program, state_dir)| {
                sheprd_client::relay_host::RelayProxy { program, state_dir }
            });
            sheprd_client::relay_host::set_relay_proxy(proxy);
        })
        .detach();
}

/// A running `desktop-pair`: its lines arrive on `outputs`. Dropping it
/// closes its input, which cancels, and kills and waits for that exact
/// child on its own thread.
pub(crate) struct Pairing {
    pub(crate) outputs: Receiver<PairOutput>,
    inputs: SyncSender<String>,
    /// Dropped with the pairing, which wakes the reaper.
    _stop: mpsc::Sender<()>,
}

impl Pairing {
    /// Starts `desktop-pair` naming this computer `name`. Blocking: run it
    /// off the UI thread.
    pub(crate) fn spawn(name: &str) -> Result<Self, Error> {
        let program = crate::bundled::locate(crate::remote::service::PROGRAM)
            .ok_or(Error::Missing(crate::remote::service::PROGRAM))?;
        let state_dir: PathBuf = crate::remote::state_dir().map_err(|_| Error::NoStateDir)?;
        let mut child = sheprd_client::new_command(program)
            .arg("desktop-pair")
            .arg("--name")
            .arg(name)
            .arg("--state-dir")
            .arg(state_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(Error::Spawn)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            reap(child);
            return Err(Error::Pipe("standard"));
        };
        let (outputs_in, outputs) = mpsc::sync_channel(QUEUE);
        thread::spawn(move || {
            let mut reader = io::BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match (&mut reader).take(MAX_LINE as u64 + 1).read_line(&mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) if line.len() > MAX_LINE => return,
                    Ok(_) => {}
                }
                let Ok(output) = serde_json::from_str::<PairOutput>(line.trim_end()) else {
                    continue;
                };
                if outputs_in.send(output).is_err() {
                    return;
                }
            }
        });
        let (inputs, lines) = mpsc::sync_channel::<String>(QUEUE);
        thread::spawn(move || {
            let mut stdin = stdin;
            for line in lines {
                if stdin
                    .write_all(line.as_bytes())
                    .and_then(|()| stdin.write_all(b"\n"))
                    .and_then(|()| stdin.flush())
                    .is_err()
                {
                    return;
                }
            }
            // Closing its input cancels the pairing.
        });
        let (stop, stopped) = mpsc::channel::<()>();
        thread::spawn(move || {
            let _ = stopped.recv();
            reap(child);
        });
        Ok(Self {
            outputs,
            inputs,
            _stop: stop,
        })
    }

    /// Hands the child what the other computer answered over Bluetooth.
    pub(crate) fn frame(&self, data: &[u8]) {
        let Ok(line) = serde_json::to_string(&PairInput::frame(data)) else {
            return;
        };
        if let Err(TrySendError::Full(_)) = self.inputs.try_send(line) {
            tracing::warn!("a nearby frame was dropped: the pairing is not reading");
        }
    }
}

/// Kills and waits for `child`, which this thread owns.
fn reap(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}
