//! The thread list: what `orchestration.subscribeShell` streams.
//!
//! The stream starts with a snapshot at a sequence, then sends each thread
//! and project as it changes. Only what Sheprd shows is typed; the rest of
//! a row is ignored, and items of a kind this client does not know are
//! skipped.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// The schema version of the shell snapshot this client reads.
pub const SCHEMA_VERSION: u32 = 2;

/// A project: a folder on the server's machine that threads run in.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub title: String,
    pub workspace_root: String,
}

/// A thread as the list shows it.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub provider_instance_id: Option<String>,
    /// T3's own word for where the thread is, such as `running` or
    /// `completed`.
    #[serde(default)]
    pub status: Option<String>,
    /// Set while the agent waits on the user to answer a request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_runtime_request: Option<Value>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl ThreadSummary {
    /// The agent waits on the user.
    pub fn waiting(&self) -> bool {
        self.pending_runtime_request
            .as_ref()
            .is_some_and(|request| !request.is_null())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub schema_version: u32,
    pub snapshot_sequence: u64,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub threads: Vec<ThreadSummary>,
}

/// One value of the stream.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind")]
pub enum Item {
    /// The list as a whole; or, carrying `resolvedRepositoryIdentityRoots`,
    /// a refresh of the projects' details alone, whose threads are empty and
    /// whose sequence is the list's own.
    #[serde(rename = "snapshot", rename_all = "camelCase")]
    Snapshot {
        snapshot: Snapshot,
        #[serde(default)]
        resolved_repository_identity_roots: Option<Vec<String>>,
    },
    #[serde(rename = "synchronized")]
    Synchronized,
    #[serde(rename = "thread.updated")]
    ThreadUpdated {
        sequence: u64,
        thread: ThreadSummary,
    },
    #[serde(rename = "thread.removed", rename_all = "camelCase")]
    ThreadRemoved {
        sequence: u64,
        #[serde(default)]
        thread_id: Option<String>,
    },
    #[serde(rename = "project.updated")]
    ProjectUpdated { sequence: u64, project: Project },
    #[serde(rename = "project.removed", rename_all = "camelCase")]
    ProjectRemoved {
        sequence: u64,
        #[serde(default)]
        project_id: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

/// The thread list, kept current from the stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shell {
    pub projects: BTreeMap<String, Project>,
    /// The sequence the list is current to; `None` before the snapshot.
    pub sequence: Option<u64>,
    pub threads: BTreeMap<String, ThreadSummary>,
}

impl Shell {
    /// Applies one stream value. A snapshot replaces the list; a change older
    /// than the list is ignored. Returns an error only for a snapshot of a
    /// schema this client cannot read.
    pub fn apply(&mut self, value: Value) -> crate::Result<()> {
        // A value this version cannot decode is skipped, as an unknown kind is.
        let Ok(item) = serde_json::from_value::<Item>(value) else {
            return Ok(());
        };
        match item {
            Item::Snapshot {
                snapshot,
                resolved_repository_identity_roots: Some(_),
            } if self.sequence.is_some() => {
                // T3's own clients merge it the same way: projects known to
                // the list take the refreshed details; nothing else changes.
                for project in snapshot.projects {
                    if let Some(known) = self.projects.get_mut(&project.id)
                        && known.workspace_root == project.workspace_root
                    {
                        *known = project;
                    }
                }
            }
            Item::Snapshot { snapshot, .. } => {
                if snapshot.schema_version != SCHEMA_VERSION {
                    return Err(crate::Error::Protocol {
                        expected: SCHEMA_VERSION,
                        found: snapshot.schema_version,
                    });
                }
                self.sequence = Some(snapshot.snapshot_sequence);
                self.projects = snapshot
                    .projects
                    .into_iter()
                    .map(|project| (project.id.clone(), project))
                    .collect();
                self.threads = snapshot
                    .threads
                    .into_iter()
                    .map(|thread| (thread.id.clone(), thread))
                    .collect();
            }
            Item::ThreadUpdated { sequence, thread } if self.fresh(sequence) => {
                self.threads.insert(thread.id.clone(), thread);
            }
            Item::ThreadRemoved {
                sequence,
                thread_id: Some(id),
            } if self.fresh(sequence) => {
                self.threads.remove(&id);
            }
            Item::ProjectUpdated { sequence, project } if self.fresh(sequence) => {
                self.projects.insert(project.id.clone(), project);
            }
            Item::ProjectRemoved {
                sequence,
                project_id: Some(id),
            } if self.fresh(sequence) => {
                self.projects.remove(&id);
            }
            _ => {}
        }
        Ok(())
    }

    /// Whether a change at `sequence` is newer than the list, and moves the
    /// list to it.
    fn fresh(&mut self, sequence: u64) -> bool {
        match self.sequence {
            Some(current) if sequence <= current => false,
            _ => {
                self.sequence = Some(sequence);
                true
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
