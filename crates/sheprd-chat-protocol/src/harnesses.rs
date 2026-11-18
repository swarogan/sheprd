//! Which agent a chat runs, while its user may still choose another.
//!
//! A chat started with `herdr-acp --choose` offers the agents installed
//! where it runs whenever its agent is idle. An agent that takes over in the
//! middle of a conversation is handed it ([`crate::handoff`]).

use serde::{Deserialize, Serialize};

use crate::Agent;

/// The agents a chat may switch to, and the one it runs now.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(from = "Names")]
pub struct Harnesses {
    /// The agents installed where the chat runs, in the table's order.
    pub available: Vec<Agent>,
    /// The agent running now.
    pub current: Option<Agent>,
}

impl Harnesses {
    /// The agent `step` places after (or, negative, before) the current one,
    /// wrapping around; `None` when there is nothing else to choose.
    pub fn step(&self, step: isize) -> Option<Agent> {
        if self.available.len() < 2 {
            return None;
        }
        let len = self.available.len() as isize;
        let at = self
            .current
            .and_then(|current| self.available.iter().position(|agent| *agent == current));
        let next = at.map_or(0, |at| (at as isize + step).rem_euclid(len)) as usize;
        self.available.get(next).copied()
    }
}

/// What a newer program may send: agents this version does not know are
/// left out rather than failing the whole transcript.
#[derive(Deserialize)]
struct Names {
    #[serde(default)]
    available: Vec<String>,
    #[serde(default)]
    current: Option<String>,
}

impl From<Names> for Harnesses {
    fn from(names: Names) -> Self {
        Self {
            available: names
                .available
                .iter()
                .filter_map(|name| name.parse().ok())
                .collect(),
            current: names.current.and_then(|name| name.parse().ok()),
        }
    }
}
