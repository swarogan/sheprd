//! Add Device's **Nearby Sheprd**: another computer's Sheprd within
//! Bluetooth range that is on another network, with no route between the
//! two (`docs/relay-plan.md`, Adding a nearby Sheprd from another network).
//!
//! The other computer opens Remote control > Pair device. Here, the
//! Bluetooth helper scans ([`Station::scan`]) and lists the computers it
//! finds; picking one starts `sheprd-remote-control desktop-pair`
//! ([`Pairing`]), whose frames this page carries over Bluetooth. Both
//! screens then show one code; once the other computer's user allows it,
//! the machine is checked through its relays and saved as a device.
//! Nothing here holds a key or decides anything: the child and the other
//! computer's daemon do.

mod render;

use super::Page;
use crate::SheprdWindow;
use crate::i18n::fl;
use crate::remote::nearby::{self, Event, Radio, Scanner, Station, desktop::Pairing};
use gpui::{prelude::*, *};
use sheprd_client::{HostProbe, SavedHost};
use sheprd_remote_control::control::desktop::{PairOutput, frame_bytes};
use std::time::Duration;

/// How often the page reads what the helper and the child said.
const POLL: Duration = Duration::from_millis(100);
/// The most computers listed at once.
const MAX_FOUND: usize = 16;
/// The session saved devices use.
const SESSION: &str = "default";

pub(in crate::menu) struct NearbySetup {
    /// The attempt under way ([`next_attempt`]): each opening, pick, and
    /// cancel starts a new one, so a result arriving late for an earlier
    /// attempt, even from a page since closed and reopened, is dropped.
    attempt: u64,
    /// Computers showing Pair device, in the order they were found.
    found: Vec<Found>,
    /// Why the last attempt stopped, in the person's words.
    error: Option<String>,
    /// The running `desktop-pair`, while one does.
    pairing: Option<Pairing>,
    /// The radio as scanning sees it; `None` until the helper says.
    radio: Option<Radio>,
    /// Keeps the helper scanning while the page lives.
    scanner: Option<Scanner>,
    step: Step,
    /// Reads the helper and the child until the page goes.
    _poll: Option<Task<()>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Found {
    /// Core Bluetooth's id for it.
    peripheral: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Looking,
    /// Connecting over Bluetooth, then starting the child.
    Connecting(Found),
    /// The code exchange is under way.
    Exchanging(Found),
    /// Both screens show `code`; `claimed` once the other computer asks
    /// its user.
    Code {
        computer: Found,
        code: String,
        claimed: bool,
    },
    /// Allowed: checking the machine through its relays, then saving it.
    Saving(String),
}

/// A number no attempt in this process had before: windows and reopened
/// pages share it.
fn next_attempt() -> u64 {
    static ATTEMPTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    ATTEMPTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// What became of saving a paired machine.
enum Outcome {
    Saved,
    Refused(String),
}

impl NearbySetup {
    /// How many computers it found, for the rail.
    pub(in crate::menu) fn found(&self) -> usize {
        self.found.len()
    }
}

impl Step {
    /// The computer being paired with, while one is.
    fn computer(&self) -> Option<&Found> {
        match self {
            Self::Connecting(found) | Self::Exchanging(found) => Some(found),
            Self::Code { computer, .. } => Some(computer),
            Self::Looking | Self::Saving(_) => None,
        }
    }
}

impl SheprdWindow {
    /// Shows the page and starts scanning.
    pub(super) fn open_add_nearby(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scanner = Station::scan(cx);
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let alive = this.update_in(cx, |this, window, cx| this.poll_nearby(window, cx));
                if !matches!(alive, Ok(true)) {
                    return;
                }
            }
        });
        self.menu.nearby_setup = Some(NearbySetup {
            attempt: next_attempt(),
            found: Vec::new(),
            error: None,
            pairing: None,
            radio: None,
            scanner,
            step: Step::Looking,
            _poll: Some(poll),
        });
        window.focus(&self.menu.focus.clone(), cx);
        self.menu.page = Some(Page::AddNearby);
    }

    /// Reads what the helper and the child said; whether to keep polling.
    fn poll_nearby(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(setup) = &mut self.menu.nearby_setup else {
            return false;
        };
        let mut events = Vec::new();
        if let Some(scanner) = &setup.scanner {
            while let Ok(event) = scanner.events.try_recv() {
                events.push(event);
            }
        }
        let mut outputs = Vec::new();
        if let Some(pairing) = &setup.pairing {
            while let Ok(output) = pairing.outputs.try_recv() {
                outputs.push(output);
            }
        }
        let changed = !events.is_empty() || !outputs.is_empty();
        for event in events {
            self.nearby_event(event, cx);
        }
        for output in outputs {
            self.nearby_output(output, window, cx);
        }
        if changed {
            cx.notify();
        }
        true
    }

    fn nearby_event(&mut self, event: Event, cx: &mut Context<Self>) {
        let Some(setup) = &mut self.menu.nearby_setup else {
            return;
        };
        match event {
            Event::CentralRadio(radio) => setup.radio = Some(radio),
            Event::Found { peripheral, name } => {
                let known = setup
                    .found
                    .iter()
                    .any(|found| found.peripheral == peripheral);
                if !known && setup.found.len() < MAX_FOUND {
                    setup.found.push(Found { peripheral, name });
                }
            }
            Event::Connected { peripheral } => {
                let Step::Connecting(found) = &setup.step else {
                    return;
                };
                if found.peripheral != peripheral {
                    return;
                }
                let found = found.clone();
                let attempt = setup.attempt;
                setup.step = Step::Exchanging(found.clone());
                let name = nearby::host_name();
                let started = cx
                    .background_executor()
                    .spawn(async move { Pairing::spawn(&name) });
                cx.spawn(async move |this, cx| {
                    let started = started.await;
                    let _ = this.update(cx, |this, cx| {
                        let Some(setup) = &mut this.menu.nearby_setup else {
                            return;
                        };
                        if setup.attempt != attempt || setup.step != Step::Exchanging(found) {
                            return;
                        }
                        match started {
                            Ok(pairing) => setup.pairing = Some(pairing),
                            Err(error) => {
                                setup.error = Some(fl!(
                                    "devices-nearby-failed",
                                    error = crate::i18n::errors::Localize::localize(&error)
                                ));
                                this.stop_nearby_pairing(cx);
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            Event::PeripheralFrame { peripheral, data } => {
                let ours = setup
                    .step
                    .computer()
                    .is_some_and(|found| found.peripheral == peripheral);
                if let (true, Some(pairing)) = (ours, &setup.pairing) {
                    pairing.frame(&data);
                }
            }
            Event::PeripheralDisconnected { peripheral } => {
                setup.found.retain(|found| found.peripheral != peripheral);
                // Once both screens show the code, Bluetooth has done its
                // part: the claim goes through the relays.
                let lost = matches!(
                    &setup.step,
                    Step::Connecting(found) | Step::Exchanging(found) if found.peripheral == peripheral
                );
                if lost {
                    let name = setup.step.computer().map(|found| found.name.clone());
                    setup.error = Some(fl!(
                        "devices-nearby-lost",
                        computer = name.unwrap_or_default()
                    ));
                    self.stop_nearby_pairing(cx);
                }
            }
            // Pairing's and presence's own events never reach a scanner.
            _ => {}
        }
    }

    fn nearby_output(&mut self, output: PairOutput, window: &mut Window, cx: &mut Context<Self>) {
        let Some(setup) = &mut self.menu.nearby_setup else {
            return;
        };
        match output {
            PairOutput::Frame { data } => {
                let (Some(found), Some(data)) = (setup.step.computer(), frame_bytes(&data)) else {
                    return;
                };
                Station::send(cx, nearby::peripheral_frame(&found.peripheral, &data));
            }
            PairOutput::Code { code, computer } => {
                let Some(found) = setup.step.computer().cloned() else {
                    return;
                };
                setup.step = Step::Code {
                    computer: Found {
                        name: crate::notifications::safe_text(&computer, 64),
                        ..found
                    },
                    code,
                    claimed: false,
                };
            }
            PairOutput::Claimed => {
                if let Step::Code { claimed, .. } = &mut setup.step {
                    *claimed = true;
                }
            }
            PairOutput::Paired { target, label } => {
                let label = crate::notifications::safe_text(&label, 64);
                if let Some(found) = setup.step.computer() {
                    Station::send(cx, nearby::disconnect(&found.peripheral));
                }
                setup.pairing = None;
                setup.step = Step::Saving(label.clone());
                self.save_nearby(target, label, window, cx);
            }
            PairOutput::Failed { message } => {
                setup.error = Some(fl!(
                    "devices-nearby-failed",
                    error = crate::notifications::safe_text(&message, 300)
                ));
                self.stop_nearby_pairing(cx);
            }
        }
    }

    /// Picks a found computer: connects to it over Bluetooth, and the code
    /// exchange starts once it is ready.
    fn pair_nearby(&mut self, peripheral: &str, cx: &mut Context<Self>) {
        let Some(setup) = &mut self.menu.nearby_setup else {
            return;
        };
        if setup.step != Step::Looking {
            return;
        }
        let Some(found) = setup
            .found
            .iter()
            .find(|found| found.peripheral == peripheral)
            .cloned()
        else {
            return;
        };
        setup.error = None;
        setup.attempt = next_attempt();
        Station::send(cx, nearby::connect(&found.peripheral));
        setup.step = Step::Connecting(found);
        cx.notify();
    }

    /// Stops the pairing under way, if any, and goes back to the list.
    fn stop_nearby_pairing(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = &mut self.menu.nearby_setup else {
            return;
        };
        if let Some(found) = setup.step.computer() {
            Station::send(cx, nearby::disconnect(&found.peripheral));
        }
        // Dropping the child closes its input, which cancels. A save under
        // way still finishes, as the other computer already allowed it, but
        // its answer no longer moves this page.
        setup.pairing = None;
        setup.attempt = next_attempt();
        setup.step = Step::Looking;
        cx.notify();
    }

    /// Checks the paired machine through its relays and saves it.
    fn save_nearby(
        &mut self,
        target: String,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let attempt = self
            .menu
            .nearby_setup
            .as_ref()
            .map_or(0, |setup| setup.attempt);
        let saved = cx
            .background_executor()
            .spawn(async move { save(&target, &label) });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = saved.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(setup) = &mut this.menu.nearby_setup else {
                    return;
                };
                if setup.attempt != attempt {
                    return;
                }
                match outcome {
                    Outcome::Saved if this.menu.page != Some(Page::AddNearby) => {
                        this.menu.nearby_setup = None;
                        cx.notify();
                    }
                    Outcome::Saved => this.dismiss_menu(window, cx),
                    Outcome::Refused(message) => {
                        setup.step = Step::Looking;
                        setup.error = Some(message);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(in crate::menu) fn add_nearby_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" => {
                let pairing = self
                    .menu
                    .nearby_setup
                    .as_ref()
                    .is_some_and(|setup| setup.step != Step::Looking);
                if pairing {
                    self.stop_nearby_pairing(cx);
                } else {
                    self.dismiss_menu(window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        window.prevent_default();
    }
}

/// Checks Herdr on the paired machine through its relays and saves it when
/// a compatible copy, or herdr-core, answers. A stopped server is fine: the
/// bridge starts it on the first connection. The same machine paired again
/// keeps its device. Blocking: runs on the background executor.
fn save(target: &str, label: &str) -> Outcome {
    match sheprd_client::probe_host(target, SESSION) {
        Ok(HostProbe::Core | HostProbe::Running | HostProbe::Stopped) => {
            let host = SavedHost {
                id: uuid::Uuid::new_v4().simple().to_string(),
                label: label.to_owned(),
                target: target.to_owned(),
                session: SESSION.to_owned(),
                enabled: true,
            };
            match sheprd_client::store_saved_host(false, &host) {
                Ok(_) => Outcome::Saved,
                Err(error) => {
                    // The log's context.
                    // i18n: data
                    crate::storage_warning::warn_storage_failure("Save nearby device", &error);
                    Outcome::Refused(fl!(
                        "devices-nearby-save-failed",
                        computer = label,
                        error = error.to_string()
                    ))
                }
            }
        }
        Ok(HostProbe::Missing | HostProbe::Outdated) => {
            Outcome::Refused(fl!("devices-nearby-no-herdr", computer = label))
        }
        Ok(HostProbe::SshFailed(_)) => {
            Outcome::Refused(fl!("devices-nearby-no-answer", computer = label))
        }
        Err(error) => Outcome::Refused(fl!(
            "devices-nearby-save-failed",
            computer = label,
            error = error.to_string()
        )),
    }
}

#[cfg(test)]
mod tests;
