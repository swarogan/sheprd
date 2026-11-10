//! Reading a Jira source with the sign-in saved for its site in the account
//! menu, read once and again after Refresh.

use super::{Result, SourceKey, SyncState, Worker, jira};
use chrono::Utc;
use std::sync::{Arc, atomic::Ordering};

impl Worker {
    /// The saved Jira sites, read from the store the first time. Blocks on
    /// the store, so only the worker calls it.
    pub(super) fn jira_record(&mut self) -> Arc<jira::saved::Saved> {
        let store = self.request.jira_store;
        self.jira_saved
            .get_or_insert_with(|| Arc::new(jira::saved::load(store)))
            .clone()
    }

    /// Syncs Jira source `key`, storing a complete listing when the
    /// database is `writable` and showing any other without storing it.
    pub(super) fn sync_jira(&mut self, key: &SourceKey, writable: bool) -> Result<SyncState> {
        let Some(store) = self.store.as_mut() else {
            return Ok(SyncState::Unsupported);
        };
        let previous = store.checkpoint(key)?;
        let saved = self.jira_record();
        let cancelled = &self.cancelled;
        let listing = jira::sync(
            key,
            saved.get(&key.host),
            previous.as_ref(),
            &|| cancelled.load(Ordering::Relaxed),
            &mut self.cooldown,
        )?;
        let Some(store) = self.store.as_mut() else {
            return Ok(SyncState::Unsupported);
        };
        let count = listing.items.len();
        if listing.viewer.is_some() {
            self.snapshot.viewer.clone_from(&listing.viewer);
        }
        if !listing.complete || !writable {
            self.uncached.insert(key.clone(), listing.items);
            return Ok(if listing.complete {
                SyncState::Synced {
                    at: Utc::now(),
                    count,
                }
            } else {
                SyncState::Capped { count }
            });
        }
        store.replace_items(key, &listing.items, &listing.checkpoint)?;
        self.uncached.remove(key);
        Ok(SyncState::Synced {
            at: Utc::now(),
            count,
        })
    }
}
