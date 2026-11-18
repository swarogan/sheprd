//! The mode each agent's chats start in: the one the person last chose in
//! any chat with that agent, such as Claude's "Bypass Permissions". A new
//! chat switches to it once the agent lists its modes, before anything is
//! said, so the choice need not be made again in every worktree. Every
//! window shares the choices, and they survive a restart.

use crate::{SheprdWindow, i18n::Localize, state_file};
use gpui::{App, Context, Global};
use serde::{Deserialize, Serialize};
use sheprd_chat_protocol::{
    Command, EntryKind, OptionCategory, OptionKind, OptionValue, Transcript,
};
use std::{collections::BTreeMap, path::PathBuf};

/// Agents remembered; a choice for another is not kept beyond this.
const MAX_AGENTS: usize = 32;
/// Bytes in an agent's name or a mode's id. Longer ones are not kept.
const MAX_FIELD_BYTES: usize = 128;
/// Room for the most choices kept, each as long as accepted: JSON escapes at
/// most double a string's bytes, plus quotes and punctuation.
const MAX_FILE_BYTES: u64 = (MAX_AGENTS * (4 * MAX_FIELD_BYTES + 16) + 64) as u64;
const FILE: &str = "chat-modes.json";

#[derive(Clone, Default, Serialize, Deserialize)]
struct Saved {
    modes: BTreeMap<String, String>,
}

fn valid(agent: &str, mode: &str) -> bool {
    [agent, mode]
        .iter()
        .all(|field| !field.is_empty() && field.len() <= MAX_FIELD_BYTES)
}

fn parse(bytes: &[u8]) -> crate::Result<BTreeMap<String, String>> {
    let saved: Saved = serde_json::from_slice(bytes)?;
    Ok(saved
        .modes
        .into_iter()
        .filter(|(agent, mode)| valid(agent, mode))
        .take(MAX_AGENTS)
        .collect())
}

/// The mode last chosen for each agent, by the agent's name.
#[derive(Default)]
pub(crate) struct Remembered {
    modes: BTreeMap<String, String>,
    writer: Option<state_file::Writer<Saved>>,
    quitting: bool,
}

impl Global for Remembered {}

impl Remembered {
    fn path() -> Option<PathBuf> {
        crate::preferences::state_dir().map(|dir| dir.join(FILE))
    }

    /// Called before starting GPUI. A missing or damaged file remembers nothing.
    pub(crate) fn load() -> Self {
        Self::at(Self::path())
    }

    fn at(path: Option<PathBuf>) -> Self {
        let modes = path
            .as_deref()
            .map(|path| {
                state_file::read(path, MAX_FILE_BYTES)
                    .and_then(|bytes| bytes.as_deref().map_or(Ok(BTreeMap::new()), parse))
            })
            .transpose()
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Cannot restore the chat modes");
                None
            })
            .unwrap_or_default();
        let writer = path.and_then(|path| {
            state_file::Writer::start("chat-modes", path)
                .inspect_err(|error| tracing::warn!(%error, "Cannot start the chat modes worker"))
                .ok()
        });
        Self {
            modes,
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

    fn mode(&self, agent: &str) -> Option<&str> {
        self.modes.get(agent).map(String::as_str)
    }

    /// Remembers `mode` for `agent`'s next chats.
    fn set(&mut self, agent: &str, mode: &str) {
        if !valid(agent, mode)
            || self.mode(agent) == Some(mode)
            || (self.modes.len() >= MAX_AGENTS && !self.modes.contains_key(agent))
        {
            return;
        }
        self.modes.insert(agent.to_owned(), mode.to_owned());
        if self.quitting {
            return;
        }
        if let Some(writer) = &self.writer {
            writer.save(Saved {
                modes: self.modes.clone(),
            });
        }
    }

    /// [`Self::set`], creating an unsaved store for fixtures that never
    /// installed one.
    fn record(agent: &str, mode: &str, cx: &mut App) {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
        cx.global_mut::<Self>().set(agent, mode);
    }

    #[cfg(test)]
    fn finish(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.finish();
        }
    }
}

/// The mode `command` chooses in a chat showing `transcript`, if it
/// chooses one: through the agent's mode setting, or its older modes.
fn chosen<'a>(transcript: &Transcript, command: &'a Command) -> Option<&'a str> {
    match command {
        Command::SetMode { id } => Some(id),
        Command::SetOption {
            id,
            value: OptionValue::Choice { id: choice },
        } if transcript
            .option(OptionCategory::Mode)
            .is_some_and(|option| &option.id == id) =>
        {
            Some(choice)
        }
        _ => None,
    }
}

/// Whether the agent has listed modes the chat could switch between.
fn lists_modes(transcript: &Transcript) -> bool {
    transcript.modes.is_some() || transcript.option(OptionCategory::Mode).is_some()
}

/// Whether nothing was said in the chat yet: a mode chosen for new chats
/// must not change one already under way, or one only watched.
fn fresh(transcript: &Transcript) -> bool {
    // A truncated transcript had earlier entries, perhaps every prompt.
    !transcript.read_only
        && !transcript.truncated
        && transcript.turn.is_none()
        && transcript.queued.is_empty()
        && !transcript
            .entries
            .iter()
            .any(|entry| matches!(entry.kind, EntryKind::User { .. }))
}

/// The command that puts the chat in `mode`, unless it is in it already or
/// its agent offers no such mode.
fn switch_to(transcript: &Transcript, mode: &str) -> Option<Command> {
    if let Some(option) = transcript.option(OptionCategory::Mode) {
        let OptionKind::Select { choices, current } = &option.kind else {
            return None;
        };
        return (current != mode && choices.iter().any(|choice| choice.id == mode)).then(|| {
            Command::SetOption {
                id: option.id.clone(),
                value: OptionValue::Choice {
                    id: mode.to_owned(),
                },
            }
        });
    }
    let modes = transcript.modes.as_ref()?;
    (modes.current != mode && modes.available.iter().any(|known| known.id == mode)).then(|| {
        Command::SetMode {
            id: mode.to_owned(),
        }
    })
}

/// Settles a chat showing `transcript` once its agent lists its modes: a
/// fresh chat switches to the mode `remembered` holds for the agent.
/// `settled` names the agent the chat was last settled for; each agent is
/// settled once, so a chat that switches agents before its first prompt
/// starts in the new agent's mode too.
fn settle(
    transcript: &Transcript,
    settled: &mut Option<String>,
    remembered: Option<&Remembered>,
) -> Option<Command> {
    if !lists_modes(transcript) || settled.as_deref() == Some(&transcript.agent) {
        return None;
    }
    *settled = Some(transcript.agent.clone());
    let mode = remembered?.mode(&transcript.agent)?;
    fresh(transcript)
        .then(|| switch_to(transcript, mode))
        .flatten()
}

impl SheprdWindow {
    /// Remembers the mode `command` chooses in `pane_id`'s chat for the next
    /// chats with the same agent. Called as the command is sent.
    pub(super) fn remember_chat_mode(&self, pane_id: &str, command: &Command, cx: &mut App) {
        let Some(transcript) = self
            .chats
            .pane(pane_id)
            .map(|pane| pane.session.transcript())
        else {
            return;
        };
        if let Some(mode) = chosen(transcript, command) {
            Remembered::record(&transcript.agent, mode, cx);
        }
    }

    /// Runs every tick: switches each new chat to its agent's remembered mode.
    pub(super) fn settle_chat_modes(&mut self, cx: &mut Context<Self>) {
        let remembered = cx.try_global::<Remembered>();
        let mut changed = false;
        for pane in self.chats.panes_mut() {
            let Some(command) = settle(
                pane.session.transcript(),
                &mut pane.mode_settled,
                remembered,
            ) else {
                continue;
            };
            pane.error = pane.send(command).err().map(|error| error.localize());
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
