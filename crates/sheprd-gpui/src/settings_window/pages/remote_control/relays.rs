//! The relays the daemon listens on (`[remote] relays`): each with how it
//! stands, a way to take it off, a field to add one, and Sheprd's own
//! relays to pick from. Saving the list restarts the daemon, as any
//! `[remote]` change does.
// i18n: migrated

mod deploy;

use super::{Edit, SettingsWindow, ui};
use crate::{
    config::{
        Theme,
        remote::{Relay, relay_address},
    },
    i18n::{Localize, fl},
    remote::{
        Health,
        public_relays::{self, Listed, Public},
    },
};
use gpui::{prelude::*, *};
use sheprd_remote_control::control::RelayState;

/// An example address, the same in every language.
pub(super) const PLACEHOLDER: &str = "https://relay.example.org";

/// Reads Sheprd's relay list; blocking.
pub(in crate::settings_window) type Fetch = fn() -> Result<Listed, public_relays::Error>;

/// Sheprd's own relays, as far as this window knows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::settings_window) enum Offered {
    /// Not asked for yet.
    #[default]
    Unknown,
    Loading,
    Listed(Listed),
    /// Why the list offers nothing, in the user's language.
    Failed(String),
}

/// How a relay stands, as the daemon reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::settings_window) enum Standing {
    /// The daemon does not run, so it reports nothing.
    Off,
    /// With the round trip in milliseconds, once measured.
    Listening(Option<u32>),
    Connecting,
    Failed(String),
    /// Between a save and the daemon's restart, or an older daemon.
    Unreported,
}

impl Standing {
    pub(in crate::settings_window) fn of(relay: &str, health: &Health) -> Self {
        let Health::Running(status) = health else {
            return Self::Off;
        };
        // The daemon names a relay as `relay_address` writes it; a list
        // edited by hand may spell it another way.
        let reported = status.relays.get(relay).or_else(|| {
            relay_address(relay)
                .ok()
                .and_then(|address| status.relays.get(&address))
        });
        match reported.map(|reported| &reported.state) {
            Some(RelayState::Listening { rtt_ms }) => Self::Listening(*rtt_ms),
            Some(RelayState::Connecting) => Self::Connecting,
            Some(RelayState::Failed { error }) => Self::Failed(error.clone()),
            None => Self::Unreported,
        }
    }

    /// The row's description.
    pub(in crate::settings_window) fn line(&self) -> String {
        match self {
            Self::Off => fl!("settings-remote-while-running"),
            Self::Listening(None) => fl!("settings-remote-relay-listening"),
            Self::Listening(Some(ms)) => fl!("settings-remote-relay-listening-rtt", ms = ms),
            Self::Connecting => fl!("settings-remote-relay-connecting"),
            Self::Failed(error) => fl!(
                "settings-remote-relay-failed",
                error = crate::notifications::safe_text(error, 200),
            ),
            Self::Unreported => fl!("settings-remote-relay-unreported"),
        }
    }

    /// The row's dot: green while listening, red once failed, yellow while
    /// connecting, and quiet when there is nothing to say.
    fn color(&self, theme: &Theme) -> u32 {
        match self {
            Self::Listening(_) => theme.ink(theme.palette[2]),
            Self::Failed(_) => theme.ink(theme.palette[1]),
            Self::Connecting => theme.ink(theme.palette[3]),
            Self::Off | Self::Unreported => theme.muted,
        }
    }
}

/// A listed relay's description: how it stands, then, for a labelled one,
/// its address when a name took its place, its description, and its tags.
pub(in crate::settings_window) fn relay_line(relay: &Relay, standing: &Standing) -> String {
    let label = &relay.label;
    let mut parts = vec![standing.line()];
    if label.name.is_some() {
        parts.push(crate::notifications::safe_text(&relay.endpoint, 200));
    }
    if let Some(description) = &label.description {
        parts.push(crate::notifications::safe_text(description, 256));
    }
    if !label.tags.is_empty() {
        let tags = label
            .tags
            .iter()
            .map(|tag| crate::notifications::safe_text(tag, 32))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(fl!("settings-remote-relay-tags", tags = tags));
    }
    parts.join(" · ")
}

/// A public relay's title: its name and region when the list gives them,
/// else its address.
pub(in crate::settings_window) fn public_title(relay: &Public) -> String {
    let text = |label: &Option<String>| {
        label
            .as_deref()
            .map(|label| crate::notifications::safe_text(label, 64))
    };
    match (text(&relay.name), text(&relay.region)) {
        (Some(name), Some(region)) => fl!(
            "settings-remote-public-relay-named",
            name = name,
            region = region
        ),
        (Some(label), None) | (None, Some(label)) => label,
        (None, None) => crate::notifications::safe_text(&relay.address, 200),
    }
}

impl SettingsWindow {
    /// Whether `address` is already in the list, however it is spelled.
    fn lists_relay(&self, address: &str) -> bool {
        self.config
            .remote
            .relays
            .iter()
            .any(|Relay { endpoint, .. }| {
                endpoint == address || relay_address(endpoint).is_ok_and(|it| it == address)
            })
    }

    fn save_with_relay(&mut self, address: String, cx: &mut Context<Self>) {
        if self.lists_relay(&address) {
            return;
        }
        let mut relays = self.config.remote.relays.clone();
        relays.push(address.into());
        self.save_remote(Edit::Relays(relays), cx);
    }

    /// Add's click: the address typed joins the list, unless it is not a
    /// relay's address. One already listed only clears the field.
    pub(in crate::settings_window) fn add_remote_relay(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.remote.relay.clone() else {
            return;
        };
        let Ok(address) = relay_address(input.read(cx).text()) else {
            self.remote.relay_invalid = true;
            cx.notify();
            return;
        };
        self.remote.relay_invalid = false;
        input.update(cx, |input, cx| input.clear(cx));
        self.save_with_relay(address, cx);
        cx.notify();
    }

    /// Add's click on the relay at `index` of Sheprd's list.
    pub(in crate::settings_window) fn add_public_relay(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        self.add_public_relay_at(index, public_relays::now(), cx);
    }

    /// [`Self::add_public_relay`] at `now` (Unix seconds).
    pub(in crate::settings_window) fn add_public_relay_at(
        &mut self,
        index: usize,
        now: u64,
        cx: &mut Context<Self>,
    ) {
        if self.expire_public_relays(now, cx) {
            return;
        }
        let Offered::Listed(listed) = &self.remote.offered else {
            return;
        };
        if let Some(relay) = listed.relays.get(index) {
            self.save_with_relay(relay.address.clone(), cx);
        }
    }

    /// Stops offering a list that expired by `now` (Unix seconds), however
    /// long the window stayed open: its relays go, and the page says why,
    /// with Retry. True when it did.
    pub(in crate::settings_window) fn expire_public_relays(
        &mut self,
        now: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let Offered::Listed(listed) = &self.remote.offered else {
            return false;
        };
        if !listed.expired(now) {
            return false;
        }
        let expired = sheprd_relay::directory::Error::Expired {
            expires: listed.expires,
        };
        self.remote.offered = Offered::Failed(public_relays::Error::from(expired).localize());
        cx.notify();
        true
    }

    /// Remove's click on the relay at `index`.
    pub(in crate::settings_window) fn remove_remote_relay(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let mut relays = self.config.remote.relays.clone();
        if index < relays.len() {
            relays.remove(index);
            self.save_remote(Edit::Relays(relays), cx);
        }
    }

    /// Reads Sheprd's relay list with `fetch`, off the UI thread. One
    /// read runs at a time.
    pub(in crate::settings_window) fn load_public_relays(
        &mut self,
        fetch: Fetch,
        cx: &mut Context<Self>,
    ) {
        if self.remote.offered == Offered::Loading {
            return;
        }
        self.remote.offered = Offered::Loading;
        cx.notify();
        let work = cx.background_executor().spawn(async move { fetch() });
        cx.spawn(async move |this, cx| {
            let listed = work.await;
            let _ = this.update(cx, |this, cx| {
                this.remote.offered = match listed {
                    Ok(listed) => Offered::Listed(listed),
                    Err(error) => {
                        tracing::warn!(%error, "Could not read Sheprd's relay list");
                        Offered::Failed(error.localize())
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn remote_relays(&self, health: &Health, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let relays = &self.config.remote.relays;
        let idle = !self.busy();
        let rows = relays.iter().enumerate().map(|(index, relay)| {
            let address = crate::notifications::safe_text(&relay.endpoint, 200);
            let name = relay.label.name.as_deref().map_or_else(
                || address.clone(),
                |name| crate::notifications::safe_text(name, 64),
            );
            let standing = Standing::of(&relay.endpoint, health);
            let remove = ui::button(
                theme,
                format!("remote-relay-{index}-remove"),
                fl!("settings-remote-relay-remove"),
                idle,
            )
            .aria_label(fl!(
                "settings-remote-relay-remove-label",
                relay = name.clone()
            ))
            .when(idle, |button| {
                button.on_click(
                    cx.listener(move |this, _, _, cx| this.remove_remote_relay(index, cx)),
                )
            });
            ui::row(theme, format!("remote-relay-{index}"), name)
                .dot(standing.color(theme))
                .description(relay_line(relay, &standing))
                .control(remove)
                .into_any_element()
        });
        let add = ui::button(
            theme,
            "remote-relay-add-button",
            fl!("settings-remote-relay-add-button"),
            idle,
        )
        .when(idle, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.add_remote_relay(cx)))
        });
        let field = div()
            .debug_selector(|| "remote-relay-address".into())
            .children(self.remote.relay.clone());
        let none = relays.is_empty();
        let list = div().flex().flex_col().child(
            ui::row(theme, "remote-relays", fl!("settings-remote-relays")).description(if none {
                fl!("settings-remote-relays-none")
            } else {
                fl!("settings-remote-relays-note")
            }),
        );
        // No relay yet: deploying one and adding one sit side by side.
        // Once one is listed, Add keeps its row and deploying moves under it.
        let list = if none {
            list.child(self.relay_start(field, add))
        } else {
            list.children(rows)
                .child(
                    ui::row(theme, "remote-relay-add", fl!("settings-remote-relay-add"))
                        .description(if self.remote.relay_invalid {
                            fl!("settings-remote-relay-invalid", example = PLACEHOLDER)
                        } else {
                            fl!("settings-remote-relay-add-note")
                        })
                        .control(field.w(px(260.)).flex_none())
                        .control(add),
                )
                .child(self.relay_deploy_row())
        };
        div()
            .flex()
            .flex_col()
            .child(ui::section(
                theme,
                &fl!("settings-remote-section-relays"),
                None,
            ))
            .child(list)
            .child(self.remote_public_relays(cx))
    }

    /// Sheprd's own relays: what the signed list offers, each with Add.
    fn remote_public_relays(&self, cx: &mut Context<Self>) -> Div {
        let theme = &self.theme;
        let idle = !self.busy();
        let header = ui::row(
            theme,
            "remote-public-relays",
            fl!("settings-remote-public-relays"),
        );
        let setup = || {
            ui::button(
                theme,
                "remote-public-relays-setup",
                fl!("settings-remote-public-relays-setup"),
                true,
            )
            .on_click(|_, _, cx| cx.open_url(sheprd_relay::directory::SETUP_URL))
        };
        let header = match &self.remote.offered {
            Offered::Unknown | Offered::Loading => {
                header.description(fl!("settings-remote-public-relays-loading"))
            }
            Offered::Failed(reason) => header.description(reason.clone()).control(
                ui::button(
                    theme,
                    "remote-public-relays-retry",
                    fl!("settings-remote-retry"),
                    true,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.load_public_relays(public_relays::fetch, cx);
                })),
            ),
            Offered::Listed(listed) if listed.relays.is_empty() => header
                .description(fl!("settings-remote-public-relays-none"))
                .control(setup()),
            Offered::Listed(listed) => header.description(fl!(
                "settings-remote-public-relays-validated",
                signer = listed.signer.describe(),
            )),
        };
        let offered = match &self.remote.offered {
            Offered::Listed(listed) => listed.relays.as_slice(),
            _ => &[],
        };
        let rows = offered.iter().enumerate().map(|(index, relay)| {
            let added = self.lists_relay(&relay.address);
            let title = public_title(relay);
            let button = ui::button(
                theme,
                format!("remote-public-relay-{index}-add"),
                if added {
                    fl!("settings-remote-public-relay-added")
                } else {
                    fl!("settings-remote-relay-add-button")
                },
                idle && !added,
            )
            .aria_label(fl!(
                "settings-remote-public-relay-add-label",
                relay = title.clone()
            ))
            .when(idle && !added, |button| {
                button.on_click(cx.listener(move |this, _, _, cx| this.add_public_relay(index, cx)))
            });
            ui::row(theme, format!("remote-public-relay-{index}"), title)
                .description(crate::notifications::safe_text(&relay.address, 200))
                .control(button)
                .into_any_element()
        });
        div().flex().flex_col().child(header).children(rows)
    }
}
