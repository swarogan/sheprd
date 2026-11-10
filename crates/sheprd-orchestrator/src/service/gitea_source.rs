//! Reading a Gitea source with the address and token saved for its host in
//! the account menu, read once and again after Refresh.

use super::{Result, SourceKey, SyncState, Worker, gitea};
use chrono::Utc;
use std::sync::{Arc, atomic::Ordering};

impl Worker {
    /// The saved Gitea instances, read from the store the first time. Blocks
    /// on the store, so only the worker calls it.
    pub(super) fn gitea_record(&mut self) -> Arc<gitea::saved::Saved> {
        let store = self.request.gitea_store;
        self.gitea_saved
            .get_or_insert_with(|| Arc::new(gitea::saved::load(store)))
            .clone()
    }

    /// Syncs Gitea source `key`, storing a complete listing when the
    /// database is `writable` and showing any other without storing it.
    pub(super) fn sync_gitea(&mut self, key: &SourceKey, writable: bool) -> Result<SyncState> {
        let Some(store) = self.store.as_mut() else {
            return Ok(SyncState::Unsupported);
        };
        let previous = store.checkpoint(key)?;
        let saved = self.gitea_record();
        let cancelled = &self.cancelled;
        let listing = gitea::sync(
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
        // This source's own pull requests, as the listing matched them.
        let prefix = format!("{}:", key.canonical());
        let mut authored = (*self.snapshot.authored).clone();
        authored.retain(|item| !item.starts_with(&prefix));
        authored.extend(listing.authored.iter().cloned());
        self.snapshot.authored = Arc::new(authored);
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
