//! A phone's change to the layout of one of Sheprd's Herdr 2 tabs: a split,
//! a zoom, or closing it, or to a workspace Sheprd keeps: renaming or
//! closing it, or deleting its linked worktree. herdr-core keeps no layout; Sheprd keeps it in its saved tabs
//! and changes it as the desktop does, so the phone never writes it and
//! never fights the desktop over it. The question waits in the daemon with
//! the phones' questions about workspaces (`control::changes`), and Sheprd
//! answers it with `Request::LayoutAnswer`.

use super::changes::AskId;
use serde::{Deserialize, Serialize};

/// The longest terminal address sent.
pub const MAX_TERMINAL: usize = 256;
/// The longest name a rename sends, in bytes; Sheprd trims it further.
pub const MAX_NAME: usize = 1024;

/// One change to one of Sheprd's Herdr 2 tabs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ask {
    pub id: AskId,
    /// The saved tab, by the id Sheprd gave it; 0 for a workspace's change.
    pub tab: u64,
    /// The pane's terminal, by its herdr-core address; empty for a
    /// workspace's change.
    pub terminal: String,
    pub op: Op,
}

impl Ask {
    /// Whether it stays within this module's bounds.
    pub fn valid(&self) -> bool {
        let names_pane = !self.terminal.is_empty() && self.terminal.len() <= MAX_TERMINAL;
        let names_workspace =
            |workspace: &str| !workspace.is_empty() && workspace.len() <= MAX_TERMINAL;
        match &self.op {
            Op::CloseWorkspace { workspace } | Op::RemoveWorktree { workspace } => {
                names_workspace(workspace)
            }
            Op::RenameWorkspace { workspace, name } => {
                names_workspace(workspace) && !name.trim().is_empty() && name.len() <= MAX_NAME
            }
            Op::RenamePane(name) | Op::RenameTab(name) => {
                names_pane && name.as_ref().is_none_or(|name| name.len() <= MAX_NAME)
            }
            Op::Close | Op::SplitDown | Op::SplitRight | Op::Zoom => names_pane,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// A new shell beside the pane, as Split Right does on the desktop.
    SplitRight,
    /// A new shell below it.
    SplitDown,
    /// The pane fills its tab, or the split shows again.
    Zoom,
    /// The tab goes. The daemon ends its terminals; Sheprd drops the tab,
    /// which would otherwise keep its last pane, ended.
    Close,
    /// Names the pane, as a chat's label; `None` gives back its title.
    RenamePane(Option<String>),
    /// Names the tab; `None` gives back its program's title.
    RenameTab(Option<String>),
    /// Names a workspace Sheprd keeps for Herdr 2, which has no tab here.
    RenameWorkspace { workspace: String, name: String },
    /// Closes a workspace Sheprd keeps for Herdr 2: its tabs close and
    /// their terminals end, as its menu's Close does.
    CloseWorkspace { workspace: String },
    /// Deletes the linked worktree of a workspace Sheprd keeps for Herdr 2,
    /// then closes the workspace. Never forced: a checkout with changes
    /// stays.
    RemoveWorktree { workspace: String },
}

/// Sheprd's answer to an [`Ask`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer", content = "detail")]
pub enum Answer {
    Done,
    Failed(Failure),
    /// A split landed: the new shell's terminal, by its herdr-core address,
    /// which the phone shows next.
    Split(String),
}

/// Why Sheprd did not change the tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    #[error("Sheprd is starting another terminal; try again in a moment")]
    Busy,
    #[error("no Sheprd window has this tab open")]
    Gone,
    #[error("this checkout has uncommitted changes; delete it in Sheprd to discard them")]
    Dirty,
    #[error("this workspace is not a linked worktree Sheprd can delete")]
    NotWorktree,
    #[error("this tab only watches its terminal; split it where it types")]
    Watching,
    /// Last: what a newer Sheprd answers that this daemon does not know.
    #[error("Sheprd could not change this tab")]
    #[serde(other)]
    Refused,
}
