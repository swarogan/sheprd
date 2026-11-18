//! Where a new worktree lands: its shell, an agent the window starts in
//! that shell, or a chat with an agent. The new worktree dialog offers the
//! agents installed on the checkout's host, a chat for those `herdr-acp`
//! runs when chats can start there, and the last choice made is offered
//! first next time, in every window and across runs.

use crate::{
    SheprdWindow,
    i18n::{Localize, fl},
    state::Answer,
    state_file,
    teleport::{AgentKind, Host},
};
use gpui::{
    AnyElement, App, AppContext, Context, Global, InteractiveElement, IntoElement, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Window, div,
    prelude::FluentBuilder, px, rgb,
};
use serde::{Deserialize, Serialize};
use sheprd_chat_protocol::Agent;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// The file only ever holds one short agent name and a flag.
const MAX_FILE_BYTES: u64 = 256;
const FILE: &str = "worktree-landing.json";
/// The width of each row's label, so both rows' tiles start at one edge.
const LINE_LABEL_WIDTH: f32 = 76.;

/// What a new worktree's first pane shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Landing {
    /// The shell Herdr opens in the checkout.
    #[default]
    Terminal,
    /// `kind`, started in that shell.
    Agent(AgentKind),
    /// A chat with the agent, `herdr-acp` started in that shell.
    Chat(Agent),
}

impl Landing {
    fn agent(self) -> Option<AgentKind> {
        match self {
            Self::Terminal | Self::Chat(_) => None,
            Self::Agent(kind) => Some(kind),
        }
    }

    /// The installed agent this landing needs, a chat's by its CLI.
    fn needs(self) -> Option<AgentKind> {
        match self {
            Self::Terminal => None,
            Self::Agent(kind) => Some(kind),
            Self::Chat(agent) => AgentKind::parse(agent.name()),
        }
    }
}

/// The chat `herdr-acp` runs for `kind`, if it runs one.
fn chat_agent(kind: AgentKind) -> Option<Agent> {
    kind.name().parse().ok()
}

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// The agent kind's name; see [`AgentKind::parse`]. Absent for the terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    /// Whether the agent opens as a chat. Absent, as older files have it,
    /// for an agent in the terminal.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    chat: bool,
}

impl From<Landing> for Saved {
    fn from(landing: Landing) -> Self {
        let (agent, chat) = match landing {
            Landing::Terminal => (None, false),
            Landing::Agent(kind) => (Some(kind.name()), false),
            Landing::Chat(agent) => (Some(agent.name()), true),
        };
        Self {
            agent: agent.map(str::to_owned),
            chat,
        }
    }
}

/// An unknown agent, from a newer or edited file, lands in the terminal.
fn parse(bytes: &[u8]) -> crate::Result<Landing> {
    let saved: Saved = serde_json::from_slice(bytes)?;
    let Some(name) = saved.agent.as_deref() else {
        return Ok(Landing::Terminal);
    };
    Ok(if saved.chat {
        name.parse().map_or(Landing::Terminal, Landing::Chat)
    } else {
        AgentKind::parse(name).map_or(Landing::Terminal, Landing::Agent)
    })
}

/// The landing chosen last, kept for the next dialog.
#[derive(Default)]
pub(crate) struct Remembered {
    landing: Landing,
    writer: Option<state_file::Writer<Saved>>,
    quitting: bool,
}

impl Global for Remembered {}

impl Remembered {
    fn path() -> Option<PathBuf> {
        crate::preferences::state_dir().map(|dir| dir.join(FILE))
    }

    /// Called before starting GPUI. A missing or damaged file lands in the terminal.
    pub(crate) fn load() -> Self {
        Self::at(Self::path())
    }

    fn at(path: Option<PathBuf>) -> Self {
        let landing = path
            .as_deref()
            .map(|path| {
                state_file::read(path, MAX_FILE_BYTES)
                    .and_then(|bytes| bytes.as_deref().map_or(Ok(Landing::Terminal), parse))
            })
            .transpose()
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Cannot restore the new worktree landing");
                None
            })
            .unwrap_or_default();
        let writer = path.and_then(|path| {
            state_file::Writer::start("worktree-landing", path)
                .inspect_err(|error| {
                    tracing::warn!(%error, "Cannot start the worktree landing worker");
                })
                .ok()
        });
        Self {
            landing,
            writer,
            quitting: false,
        }
    }

    pub(crate) fn install(self, cx: &mut App) {
        cx.set_global(self);
        cx.on_app_quit(|cx| {
            let remembered = cx.global_mut::<Self>();
            remembered.quitting = true;
            let writer = remembered.writer.take();
            cx.background_executor().spawn(async move {
                if let Some(writer) = writer {
                    writer.finish();
                }
            })
        })
        .detach();
    }

    /// The landing the next dialog offers first.
    pub(crate) fn landing(cx: &App) -> Landing {
        cx.try_global::<Self>()
            .map_or(Landing::Terminal, |remembered| remembered.landing)
    }

    /// Remember `landing`, creating an unsaved store for fixtures that never
    /// installed one.
    pub(crate) fn record(landing: Landing, cx: &mut App) {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
        let remembered = cx.global_mut::<Self>();
        if remembered.landing == landing {
            return;
        }
        remembered.landing = landing;
        if remembered.quitting {
            return;
        }
        if let Some(writer) = &remembered.writer {
            writer.save(landing.into());
        }
    }

    #[cfg(test)]
    fn finish(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.finish();
        }
    }
}

/// The dialog's landing rows: what is chosen, and the agents its host has.
pub(crate) struct Picker {
    pub(crate) chosen: Landing,
    /// Whether this window can start a chat on the host, so the chat row
    /// shows.
    chats: bool,
    /// The installed agents, in [`AgentKind::ALL`] order; `None` while the
    /// host is being looked at.
    installed: Option<Vec<AgentKind>>,
    host: Host,
    /// Set when the dialog closes, ending a lookup still running.
    cancelled: Arc<AtomicBool>,
}

impl Drop for Picker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Picker {
    /// A remembered chat, where no chat can start, lands in its agent's
    /// terminal instead.
    pub(crate) fn new(host: Host, chosen: Landing, chats: bool) -> Self {
        let mut picker = Self {
            chosen,
            chats,
            installed: None,
            host,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        picker.allow_chats(chats);
        picker
    }

    /// Show or hide the chat row, as whether a chat can start changes while
    /// the dialog is open. A chosen chat that can no longer start lands in
    /// its agent's terminal, or the shell where the host lacks the agent.
    pub(crate) fn allow_chats(&mut self, chats: bool) {
        self.chats = chats;
        if chats || !matches!(self.chosen, Landing::Chat(_)) {
            return;
        }
        self.chosen = self
            .chosen
            .needs()
            .filter(|kind| {
                self.installed
                    .as_ref()
                    .is_none_or(|installed| installed.contains(kind))
            })
            .map_or(Landing::Terminal, Landing::Agent);
    }

    /// Whether the host is still being looked at.
    pub(crate) fn looking(&self) -> bool {
        self.installed.is_none()
    }

    /// Take the host's agents. One the host lacks cannot be chosen, so a
    /// remembered one that is missing lands in the terminal instead. A host
    /// that could not be looked at offers every agent rather than none; the
    /// start then reports one that is missing.
    pub(crate) fn offer(&mut self, installed: sheprd_client::Result<Vec<AgentKind>>) {
        let installed = installed.unwrap_or_else(|error| {
            tracing::warn!(%error, "Cannot look up the host's agents");
            AgentKind::ALL.to_vec()
        });
        if self
            .chosen
            .needs()
            .is_some_and(|kind| !installed.contains(&kind))
        {
            self.chosen = Landing::Terminal;
        }
        self.installed = Some(installed);
    }

    /// The landings to show, the terminal first. While the host is looked
    /// at, the chosen agent shows alone so the remembered pick is visible.
    pub(crate) fn offered(&self) -> Vec<Landing> {
        let agents = match &self.installed {
            Some(installed) => installed.clone(),
            None => self.chosen.agent().into_iter().collect(),
        };
        std::iter::once(Landing::Terminal)
            .chain(agents.into_iter().map(Landing::Agent))
            .collect()
    }

    /// The chats to show, in [`AgentKind::ALL`] order: one per installed
    /// agent `herdr-acp` runs, none where no chat can start. While the host
    /// is looked at, a chosen chat shows alone.
    pub(crate) fn offered_chats(&self) -> Vec<Landing> {
        if !self.chats {
            return Vec::new();
        }
        match &self.installed {
            Some(installed) => installed
                .iter()
                .copied()
                .filter_map(chat_agent)
                .map(Landing::Chat)
                .collect(),
            None => match self.chosen {
                Landing::Chat(_) => vec![self.chosen],
                _ => Vec::new(),
            },
        }
    }

    /// Choose `landing` when it is offered.
    pub(crate) fn choose(&mut self, landing: Landing) -> bool {
        let offered = self.offered().contains(&landing) || self.offered_chats().contains(&landing);
        if offered {
            self.chosen = landing;
        }
        offered
    }
}

/// The agents installed on `host`, in [`AgentKind::ALL`] order.
fn installed(host: &Host, cancelled: &AtomicBool) -> sheprd_client::Result<Vec<AgentKind>> {
    let binaries = AgentKind::ALL.map(AgentKind::binary);
    let found = host.installed_programs(&binaries, cancelled)?;
    Ok(AgentKind::ALL
        .into_iter()
        .filter(|kind| found.iter().any(|binary| binary == kind.binary()))
        .collect())
}

/// A Herdr agent name for `kind` that no other agent uses: Herdr's names
/// start with a lowercase letter and hold at most 32 lowercase letters,
/// digits, and dashes.
fn agent_name(kind: AgentKind) -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("{}-{}", kind.name(), &hex[..8])
}

/// A new worktree's landing, kept from its creation request until the
/// daemon answers, so closing the dialog meanwhile does not drop it.
#[derive(Debug)]
pub(crate) struct Launch {
    /// The correlated `worktree.create` this landing is for.
    request: String,
    /// The selection epoch and connection generation it was sent under.
    endpoint: (u64, u64),
    boot: String,
    landing: Landing,
    host: Host,
}

impl Launch {
    /// `picker`'s choice for creation request `request`, sent on boot `boot`
    /// under the `endpoint` fence.
    pub(crate) fn new(request: &str, endpoint: (u64, u64), boot: &str, picker: &Picker) -> Self {
        Self {
            request: request.to_owned(),
            endpoint,
            boot: boot.to_owned(),
            landing: picker.chosen,
            host: picker.host.clone(),
        }
    }

    /// Whether an answer on this connection and boot can still be for it.
    pub(crate) fn current(&self, endpoint: (u64, u64), boot: Option<&str>) -> bool {
        self.endpoint == endpoint && boot == Some(self.boot.as_str())
    }

    /// The creation request this landing waits on.
    pub(crate) fn request(&self) -> &str {
        &self.request
    }
}

/// The created checkout's root pane and its terminal, from the daemon's
/// `worktree_created` answer.
fn root_pane(response: &serde_json::Value) -> Option<(&str, Option<&str>)> {
    let result = &response["result"];
    if result["type"] != "worktree_created" {
        return None;
    }
    let pane = &result["root_pane"];
    let id = pane["pane_id"].as_str().filter(|id| !id.is_empty())?;
    Some((id, pane["terminal_id"].as_str().filter(|id| !id.is_empty())))
}

impl SheprdWindow {
    /// Re-select the captured destination only while its saved route is
    /// unchanged. A removed or edited device never falls back to Local.
    pub(crate) fn select_worktree_host(
        &mut self,
        endpoint: &str,
        target: &sheprd_client::ConnectTarget,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .endpoints
            .iter()
            .any(|known| known.id == endpoint && &known.connection.target == target)
        {
            return false;
        }
        if self.endpoints[self.selected_endpoint].id != endpoint
            && !self.select_endpoint(endpoint, cx)
        {
            return false;
        }
        self.refresh_core_session(cx);
        true
    }

    /// The landing row for a new worktree dialog on the shown host, looking
    /// up its agents off the UI thread. `None` where the host cannot be
    /// scripted, so only its shell could be offered.
    pub(crate) fn open_landing(&mut self, cx: &mut Context<Self>) -> Option<Picker> {
        let selected = &self.endpoints[self.selected_endpoint];
        if !cfg!(any(target_os = "linux", target_os = "macos")) {
            return None;
        }
        let host = crate::teleport::host_for(&selected.connection.target).ok()?;
        let picker = Picker::new(host.clone(), Remembered::landing(cx), self.may_start_chat());
        let cancelled = picker.cancelled.clone();
        let lookup = cx.background_executor().spawn({
            let cancelled = cancelled.clone();
            async move { installed(&host, &cancelled) }
        });
        cx.spawn(async move |this, cx| {
            let result = lookup.await;
            let _ = this.update(cx, |this, cx| {
                // Only the dialog that asked: a newer one has its own lookup.
                if let Some(picker) = &mut this.menu.landing
                    && Arc::ptr_eq(&picker.cancelled, &cancelled)
                {
                    picker.offer(result);
                    cx.notify();
                }
            });
        })
        .detach();
        Some(picker)
    }

    /// Brings the open dialog's chat row up to date with whether a chat can
    /// start, such as once the search for the program ends.
    pub(crate) fn refresh_landing_chats(&mut self) {
        let chats = self.may_start_chat();
        if let Some(picker) = &mut self.menu.landing {
            picker.allow_chats(chats);
        }
    }

    /// Land the checkout the open dialog's request created. Its answer
    /// also sits in the landing's own slot, which is released.
    pub(crate) fn land_created(&mut self, response: &serde_json::Value, cx: &mut Context<Self>) {
        let Some(launch) = self.landing_launch.take() else {
            return;
        };
        let _ = self.take_answer(Answer::Landing, launch.request());
        self.land(launch, response, cx);
    }

    /// Remember `launch`'s choice and start its agent in the created root
    /// pane off the UI thread, reporting a failure. A refused or unexpected
    /// answer created nothing to land in.
    pub(crate) fn land(
        &mut self,
        launch: Launch,
        response: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let Some((pane, terminal)) = root_pane(response) else {
            return;
        };
        Remembered::record(launch.landing, cx);
        if let Landing::Chat(agent) = launch.landing {
            return self.land_chat(&launch.boot, pane, agent, cx);
        }
        let Some(kind) = launch.landing.agent() else {
            return;
        };
        let (pane, terminal) = (pane.to_owned(), terminal.map(str::to_owned));
        let host = launch.host;
        let started = cx.background_executor().spawn(async move {
            sheprd_orchestrator::actions::start_agent(
                &host,
                kind,
                &agent_name(kind),
                &pane,
                terminal.as_deref(),
                &AtomicBool::new(false),
            )
        });
        cx.spawn(async move |this, cx| {
            let Err(error) = started.await else {
                return;
            };
            tracing::warn!(%error, agent = kind.name(), "Cannot start the new worktree's agent");
            let _ = this.update(cx, |this, cx| {
                this.local_error = Some(fl!(
                    "worktree-landing-failed",
                    agent = kind.name(),
                    error = error.localize()
                ));
                cx.notify();
            });
        })
        .detach();
    }
}

impl SheprdWindow {
    /// Types the line that starts a chat with `agent` into a new checkout's
    /// root pane on boot `boot`, reporting a failure. The shell stays when
    /// the chat quits, as for a chat's own tab.
    fn land_chat(&mut self, boot: &str, pane: &str, agent: Agent, cx: &mut Context<Self>) {
        let typed = self
            .chat_line(agent)
            .map_err(|error| crate::Error::from(crate::chat::Error::from(error)))
            .and_then(|line| self.type_script_line(boot, pane, &line));
        if let Err(error) = typed {
            tracing::warn!(%error, agent = agent.name(), "Cannot start the new worktree's chat");
            self.land_failed(agent.display_name(), &error, cx);
        }
    }

    fn land_failed(&mut self, agent: &str, error: &crate::Error, cx: &mut Context<Self>) {
        self.local_error = Some(fl!(
            "worktree-landing-failed",
            agent = agent,
            error = error.localize()
        ));
        cx.notify();
    }

    /// Land a checkout made for a workspace the app keeps for Herdr 2:
    /// remember the choice, then keep a workspace at `checkout` named for
    /// `label`, whose first tab is its shell, or the chosen agent typed into
    /// a herdr-core shell there by `sheprd_remote::work`, as fan-out and
    /// dispatch start theirs, so the shell stays when the agent ends.
    pub(crate) fn land_kept(
        &mut self,
        chosen: Option<Landing>,
        label: Option<String>,
        checkout: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(chosen) = chosen {
            Remembered::record(chosen, cx);
        }
        let label = label.unwrap_or_else(|| {
            checkout
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
        });
        if let Some(Landing::Chat(agent)) = chosen {
            if let Err(error) = self.new_kept_chat_workspace(agent, Some(&label), checkout, cx) {
                tracing::warn!(%error, agent = agent.name(), "Cannot start the new worktree's chat");
                let error = crate::Error::from(crate::chat::Error::from(error));
                self.land_failed(agent.display_name(), &error, cx);
            }
            return;
        }
        let Some(kind) = chosen.and_then(Landing::agent) else {
            match self.kept_workspace_at(&checkout) {
                Some(id) => {
                    self.focus_kept_workspace(&id, cx);
                }
                None => self.new_kept_workspace(Some(&label), Some(checkout), cx),
            }
            return;
        };
        let endpoint = self.endpoints[self.selected_endpoint].id.clone();
        let target = self.endpoints[self.selected_endpoint]
            .connection
            .target
            .clone();
        let launch_target = target.clone();
        let path = checkout.to_string_lossy().into_owned();
        let started = cx.background_executor().spawn(async move {
            let host = sheprd_remote::work::host_for_kept(&launch_target)?;
            let worktree = sheprd_remote::work::Worktree {
                path,
                opened: sheprd_remote::work::Opened::Herdr2,
            };
            let agent = sheprd_remote::work::Agent {
                kind,
                name: &agent_name(kind),
                model: None,
                prompt: None,
            };
            let started = sheprd_remote::work::start_agent(
                &host,
                &worktree,
                &agent,
                &AtomicBool::new(false),
            )?;
            Ok::<_, crate::Error>(started)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = started.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if !this.select_worktree_host(&endpoint, &target, cx) {
                    this.local_error = Some(crate::Error::StaleWorkspace.localize());
                    return;
                }
                let (started, error) = kept_start_outcome(result);
                if let Some(error) = error {
                    tracing::warn!(%error, agent = kind.name(), "Cannot start the new worktree's agent");
                    this.local_error = Some(fl!(
                        "worktree-landing-failed",
                        agent = kind.name(),
                        error = error.localize()
                    ));
                }
                match started {
                Some(sheprd_remote::work::Started::Herdr2 { terminal }) => {
                    let Ok(terminal) = sheprd_backend::TerminalId::new(terminal) else {
                        return;
                    };
                    let location = crate::browser::Location::core_terminal(
                        crate::core_terminals::panes::Panes::single(terminal),
                        false,
                    );
                    this.open_kept_workspace(Some(&label), &checkout, location, window, cx);
                }
                Some(sheprd_remote::work::Started::Herdr1 { .. }) => {}
                None => {
                    // The checkout is there; its shell still opens.
                    this.new_kept_workspace(Some(&label), Some(checkout), cx);
                }
            }});
        })
        .detach();
    }

    /// The dialog's landing choices as icon tiles in two rows: in a
    /// terminal, its shell then each agent the host has, and as a chat,
    /// each of those `herdr-acp` runs. One choice spans both rows. Each
    /// tile is named in a tooltip and to assistive technology; the chat
    /// row shows only where a chat can start, and a narrow dialog wraps.
    pub(crate) fn render_landing(&self, picker: &Picker, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let chats = picker.offered_chats();
        let line =
            |key: &'static str, icon: &'static str, label: String, tiles: Vec<AnyElement>| {
                div()
                    .debug_selector(move || format!("worktree-landing-{key}-row"))
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_none()
                            .w(px(LINE_LABEL_WIDTH))
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .text_color(rgb(theme.subtext()))
                            .child(crate::icons::icon(icon, 12., theme.subtext()))
                            .child(div().min_w_0().truncate().child(label)),
                    )
                    .children(tiles)
            };
        let mut terminal: Vec<AnyElement> = Vec::new();
        for landing in picker.offered() {
            terminal.push(self.render_landing_tile(picker, landing, cx));
            if landing == Landing::Terminal {
                terminal.push(
                    div()
                        .flex_none()
                        .w(px(1.))
                        .h(px(20.))
                        .bg(rgb(theme.active))
                        .into_any_element(),
                );
            }
        }
        if picker.looking() {
            terminal.push(
                div()
                    .text_color(rgb(theme.muted))
                    .child(fl!("worktree-landing-looking"))
                    .into_any_element(),
            );
        }
        let chat: Vec<AnyElement> = chats
            .iter()
            .map(|&landing| self.render_landing_tile(picker, landing, cx))
            .collect();
        div()
            .id("worktree-landing")
            .debug_selector(|| "worktree-landing".into())
            .role(Role::RadioGroup)
            .aria_label(fl!("worktree-landing-label"))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(line(
                "terminal",
                "icons/terminal.svg",
                fl!("worktree-landing-terminal"),
                terminal,
            ))
            .when(!chat.is_empty(), |rows| {
                rows.child(line(
                    "chat",
                    "icons/chat.svg",
                    fl!("worktree-landing-chat"),
                    chat,
                ))
            })
            .into_any_element()
    }

    fn render_landing_tile(
        &self,
        picker: &Picker,
        landing: Landing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &self.theme;
        let (foreground, surface) = (theme.foreground, theme.surface);
        let on = picker.chosen == landing;
        let (key, name, icon): (String, SharedString, &'static str) = match landing {
            Landing::Terminal => (
                "terminal".to_owned(),
                fl!("worktree-landing-shell").into(),
                "icons/terminal.svg",
            ),
            Landing::Agent(kind) => (
                kind.name().to_owned(),
                kind.name().into(),
                crate::icons::AgentIcon::from_identity(Some(kind.name())).path(),
            ),
            Landing::Chat(agent) => (
                format!("chat-{}", agent.name()),
                fl!("worktree-landing-chat-agent", agent = agent.display_name()).into(),
                crate::icons::AgentIcon::from_identity(Some(agent.name())).path(),
            ),
        };
        let hint = name.clone();
        let selector = format!("worktree-landing-{key}");
        div()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .role(Role::RadioButton)
            .aria_label(name)
            .aria_toggled(if on { Toggled::True } else { Toggled::False })
            .size(px(32.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(px(crate::config::corners::CONTROL))
            .border_1()
            .border_color(rgb(if on { theme.primary() } else { theme.active }))
            .when(on, |tile| tile.bg(rgb(theme.primary_wash())))
            .hover(|tile| tile.bg(rgb(theme.active)))
            .tooltip(move |_, cx| {
                cx.new(|_| crate::usage::Hint {
                    text: hint.clone(),
                    foreground,
                    surface,
                })
                .into()
            })
            .when(!on, |tile| {
                tile.cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(picker) = &mut this.menu.landing
                            && picker.choose(landing)
                        {
                            cx.notify();
                        }
                    }))
            })
            .child(crate::icons::icon(
                icon,
                18.,
                if on { foreground } else { theme.subtext() },
            ))
            .into_any_element()
    }
}

/// A failed send may have reached the agent. Preserve its known terminal
/// and show it with the error, rather than hiding it behind a fresh shell.
fn kept_start_outcome(
    result: crate::Result<sheprd_remote::work::Started>,
) -> (Option<sheprd_remote::work::Started>, Option<crate::Error>) {
    match result {
        Ok(started) => (Some(started), None),
        Err(error) => {
            let started = match &error {
                crate::Error::Remote(sheprd_remote::Error::Work(
                    sheprd_remote::work::Error::Unprompted { started, .. },
                )) => Some(started.clone()),
                _ => None,
            };
            (started, Some(error))
        }
    }
}

#[cfg(test)]
mod tests;
