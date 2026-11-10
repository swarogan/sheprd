//! Which sources a repository has, which this build reads, and the GitLab
//! CLI's say in whether a remote is a GitLab project.

use super::{
    Access, Item, Provider, RepoInfo, Result, SourceKey, SourceStatus, SyncState, Worker, azure,
    gitea, gitlab, jira,
};
use std::collections::{HashMap, HashSet};

impl SyncState {
    /// How many items the source's last sync reported, kept while it syncs
    /// again; `None` before one has, or after one failed.
    pub fn count(&self) -> Option<usize> {
        match self {
            Self::Synced { count, .. } | Self::Capped { count } => Some(*count),
            Self::Syncing { count } => *count,
            Self::SignIn | Self::Unsupported | Self::Failed(_) => None,
        }
    }
}

impl Worker {
    /// Marks `key` syncing, keeping the count its last sync reported.
    pub(super) fn set_syncing(&mut self, key: &SourceKey) {
        if let Some(status) = self.snapshot.sources.iter_mut().find(|s| s.key == *key) {
            status.state = SyncState::Syncing {
                count: status.state.count(),
            };
        }
    }

    /// Brings the store to `info`'s Beads source, then reads it again. `info`
    /// must be what was just discovered: a cached one can be older than what
    /// another checkout of the repository synced, and settling on it would
    /// forget those tasks. Failing leaves the old rows, which
    /// [`keep_current_beads`] still hides.
    pub(super) fn settle_beads(&mut self, info: &RepoInfo) -> Result<()> {
        let Some(store) = self
            .store
            .as_mut()
            .filter(|store| matches!(store.access(), Access::ReadWrite))
        else {
            return Ok(());
        };
        if let Err(error) = store.settle_beads(info.beads_source().as_ref()) {
            tracing::warn!(
                category = "orchestrator",
                %error,
                "Cannot bring the stored beads to the project's source"
            );
        }
        self.reload()
    }
}

/// Makes `info`'s remote a Gitea source when an address is saved for its
/// host, else a GitLab one when glab knows it; returns whether it changed.
/// `gitea` reads the saved addresses and `cli` glab's config, each only when
/// needed.
pub(super) fn recognize<S: std::borrow::Borrow<gitea::saved::Saved>>(
    info: &mut RepoInfo,
    gitea: impl FnOnce() -> S,
    cli: impl FnOnce() -> Vec<gitlab::cli_hosts::CliHost>,
) -> bool {
    info.remote.as_mut().is_some_and(|remote| {
        gitea::recognize(&mut remote.source, gitea) || gitlab::recognize(&mut remote.source, cli)
    })
}

/// Drops beads of any source but `info`'s. A database this build cannot
/// write still holds them after the source moves, as a `.beads/redirect`
/// added later moves it, and they would list beside the ones read now.
/// Before the repository is known, what was last listed stays.
pub(super) fn keep_current_beads(items: &mut Vec<Item>, info: Option<&RepoInfo>) {
    let Some(info) = info else {
        return;
    };
    let current = info.beads_source();
    items.retain(|item| {
        item.key.source.provider != Provider::Beads || Some(&item.key.source) == current.as_ref()
    });
}

/// Drops Jira issues of a site no longer saved, which the database still
/// holds after the site is forgotten. Until the saved sites are read, what
/// was last listed stays.
pub(super) fn keep_saved_jira(items: &mut Vec<Item>, saved: Option<&jira::saved::Saved>) {
    let Some(saved) = saved else {
        return;
    };
    items.retain(|item| {
        item.key.source.provider != Provider::Jira || saved.get(&item.key.source.host).is_some()
    });
}

impl Worker {
    /// The sources `info` has: its own, and one per saved Jira site, which
    /// is no property of the repository and so cannot come from `info`.
    /// Blocks on the credential store the first time, so only the worker
    /// calls it.
    pub(super) fn sources_of(&mut self, info: &RepoInfo) -> Vec<(SourceKey, bool)> {
        let mut all = sources(info);
        let saved = self.jira_record();
        all.extend(jira::source_keys(&saved).into_iter().map(|key| (key, true)));
        all
    }
}

/// Each of `sources`, syncing when this build can read it.
pub(super) fn statuses(sources: Vec<(SourceKey, bool)>) -> Vec<SourceStatus> {
    sources
        .into_iter()
        .map(|(key, supported)| SourceStatus {
            key,
            state: if supported {
                SyncState::Syncing { count: None }
            } else {
                SyncState::Unsupported
            },
        })
        .collect()
}

/// The sources a repository's own files give it, and whether this build can
/// read each; [`Worker::sources_of`] adds the saved Jira sites.
pub fn sources(info: &RepoInfo) -> Vec<(SourceKey, bool)> {
    let remote = info.remote.as_ref().map(|remote| {
        let supported = match remote.source.provider {
            Provider::Azure => true,
            Provider::Gitea => true,
            Provider::Github => remote.source.host == "github.com",
            Provider::Gitlab => true,
            // Jira sources come from the saved sites, not the remote.
            Provider::Jira | Provider::Beads => false,
        };
        (remote.source.clone(), supported)
    });
    remote
        .into_iter()
        .chain(info.beads_source().map(|key| (key, true)))
        .collect()
}

/// Every source whose stored items a listing shown but not stored replaces:
/// its own, and for Azure DevOps its project's too, which its work items are
/// keyed by. A listing whose project now has no open work item still hides
/// the stored ones.
pub(super) fn replaced_by(uncached: &HashMap<SourceKey, Vec<Item>>) -> HashSet<SourceKey> {
    uncached
        .keys()
        .flat_map(|key| {
            let project = (key.provider == Provider::Azure)
                .then(|| {
                    azure::Repository::of(key)
                        .ok()
                        .map(|repository| repository.project_source())
                })
                .flatten();
            std::iter::once(key.clone()).chain(project)
        })
        .collect()
}
