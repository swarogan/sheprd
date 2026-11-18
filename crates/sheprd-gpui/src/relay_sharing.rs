//! Sharing this computer's relays with every machine it connects to, so a
//! relay the user adds once in `[remote] relays` also carries phones to
//! their SSH and cloud machines (`docs/relay-plan.md`, Choosing a relay).
//!
//! Each connected machine's own daemon keeps what each computer shared
//! (`sheprd_remote_control::control::relays`), listens on it at once, and
//! leaves that machine's own `[remote] relays` alone. [`RelaySharing`]
//! offers the list whole: at once when it changes or a machine connects,
//! then now and then, in case a machine lost what it kept. A machine
//! without `sheprd-remote-control` is skipped quietly; no phone reaches it.

mod error;
mod sync;

pub use error::Error;

use crate::device_tags::Daemon;
use gpui::{App, BorrowAppContext, EntityId, Global};
use sheprd_client::ConnectTarget;
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

/// How soon a machine that did not take the relays is tried again.
const RETRY: Duration = Duration::from_secs(120);
/// How often a machine that took them is told again.
const REFRESH: Duration = Duration::from_secs(600);
/// How long a window's report counts: windows report every poll, so one
/// silent this long has closed.
const WINDOW_SILENCE: Duration = Duration::from_secs(30);

/// Where one machine stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Link {
    /// The relays it last took, whole.
    sent: Option<Vec<String>>,
    last: Option<Instant>,
    running: bool,
}

impl Link {
    /// Whether to offer `relays` now. An empty list is offered once, so a
    /// relay removed while this app was closed is withdrawn too, without
    /// asking every machine again and again when there are none.
    fn due(&self, relays: &[String], now: Instant) -> bool {
        if self.running {
            return false;
        }
        let since = |wait| {
            self.last
                .is_none_or(|last| now.duration_since(last) >= wait)
        };
        match &self.sent {
            Some(sent) if sent == relays => !relays.is_empty() && since(REFRESH),
            Some(_) => true,
            None => since(RETRY),
        }
    }
}

/// This computer's relays and each connected machine's link.
#[derive(Default)]
pub(crate) struct RelaySharing {
    /// `[remote] relays` here, once the settings were read.
    relays: Option<Vec<String>>,
    /// The name this computer shares under, which each machine keys its
    /// relays by.
    source: String,
    /// What each window reached when it last said, by window: each window
    /// reports its own machines, so one is never taken for all of them.
    windows: HashMap<EntityId, Reached>,
    links: HashMap<String, Link>,
}

/// The machines one window reached, and when it said so.
#[derive(Debug)]
struct Reached {
    machines: BTreeMap<String, ConnectTarget>,
    at: Instant,
}

impl Global for RelaySharing {}

impl RelaySharing {
    pub(crate) fn install(cx: &mut App) {
        cx.set_global(Self {
            source: crate::remote::nearby::host_name(),
            ..Self::default()
        });
        crate::config::watch::follow(
            cx,
            "remote relays", // i18n: data
            // Addresses only: a relay's label is this computer's own words.
            |config| {
                config
                    .remote
                    .relays
                    .into_iter()
                    .map(|relay| relay.endpoint)
                    .collect()
            },
            Self::set_relays,
        )
        .detach();
    }

    /// The settings named `relays`: every machine hears of a change at once.
    fn set_relays(cx: &mut App, relays: Vec<String>) {
        let changed = cx.update_global::<Self, _>(|this, _| {
            if this.relays.as_ref() == Some(&relays) {
                return false;
            }
            this.relays = Some(relays);
            true
        });
        if changed {
            Self::tick(cx);
        }
    }

    /// Tells which devices `window` can reach; this computer's own daemon
    /// follows the settings itself, so only other machines count.
    pub(crate) fn reach(cx: &mut App, window: EntityId, reachable: &BTreeMap<String, Daemon>) {
        if !cx.has_global::<Self>() {
            return;
        }
        let machines: BTreeMap<String, ConnectTarget> = reachable
            .iter()
            .filter_map(|(id, daemon)| match daemon {
                Daemon::Local => None,
                Daemon::Remote(target) => Some((id.clone(), target.clone())),
            })
            .collect();
        let now = Instant::now();
        cx.update_global::<Self, _>(|this, _| {
            this.windows.insert(window, Reached { machines, at: now });
            this.forget_unreached(now);
        });
        Self::tick(cx);
    }

    /// Every machine some window reaches now. A window that stopped saying,
    /// such as one that closed, counts no longer.
    fn machines(&self, now: Instant) -> BTreeMap<String, ConnectTarget> {
        self.windows
            .values()
            .filter(|reached| now.duration_since(reached.at) < WINDOW_SILENCE)
            .flat_map(|reached| reached.machines.clone())
            .collect()
    }

    /// Drops windows that went quiet, and resets the links of machines no
    /// window reaches, so one that comes back, maybe restarted from
    /// anything, hears the relays again. An offer still running keeps its
    /// link, so it is never started twice.
    fn forget_unreached(&mut self, now: Instant) {
        self.windows
            .retain(|_, reached| now.duration_since(reached.at) < WINDOW_SILENCE);
        let machines = self.machines(now);
        self.links
            .retain(|id, link| link.running || machines.contains_key(id));
    }

    /// Starts the offers that are due.
    fn tick(cx: &mut App) {
        let Some(this) = cx.try_global::<Self>() else {
            return;
        };
        let Some(relays) = this.relays.clone() else {
            return;
        };
        let now = Instant::now();
        let due: Vec<(String, ConnectTarget)> = this
            .machines(now)
            .into_iter()
            .filter(|(id, _)| this.links.get(id).is_none_or(|link| link.due(&relays, now)))
            .collect();
        let source = this.source.clone();
        for (id, target) in due {
            cx.update_global::<Self, _>(|this, _| {
                let link = this.links.entry(id.clone()).or_default();
                link.running = true;
                link.last = Some(now);
            });
            let offer = sheprd_remote_control::control::relays::Offer {
                source: source.clone(),
                relays: relays.clone(),
            };
            let task = cx
                .background_executor()
                .spawn(async move { sync::share(&target, offer, &AtomicBool::new(false)) });
            let sent = relays.clone();
            cx.spawn(async move |cx| {
                let answer = task.await;
                cx.update(|cx| Self::answered(cx, &id, sent, answer));
            })
            .detach();
        }
    }

    fn answered(cx: &mut App, id: &str, sent: Vec<String>, answer: Result<bool, Error>) {
        let shared = match answer {
            Ok(shared) => shared,
            Err(error) => {
                tracing::debug!(id, %error, "Could not share relays with a machine");
                false
            }
        };
        let changed = cx.update_global::<Self, _>(|this, _| {
            let Some(link) = this.links.get_mut(id) else {
                return false;
            };
            link.running = false;
            link.sent = shared.then_some(sent);
            this.relays.as_ref() != link.sent.as_ref() && shared
        });
        // The settings changed while the offer ran: the new list goes now.
        if changed {
            Self::tick(cx);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
