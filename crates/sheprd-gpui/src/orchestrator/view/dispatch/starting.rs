//! A dispatch the worker has taken but that has no run yet: the page says
//! so and offers no second dispatch of the same item.

use crate::orchestrator::Notice;
use crate::orchestrator::view::OrchestratorView;
use sheprd_orchestrator::actions::Done;

impl OrchestratorView {
    /// Marks the item with canonical key `key` as starting in the active repository.
    pub(in crate::orchestrator::view) fn mark_starting(&mut self, key: String) {
        if let Some(repo) = self.repos.get_mut(self.active) {
            repo.starting.insert(key);
        }
    }

    /// Whether a dispatch of the item is under way and shows no run yet.
    pub(in crate::orchestrator::view) fn is_starting(&self, key: &str) -> bool {
        self.repos
            .get(self.active)
            .is_some_and(|repo| repo.starting.contains(key))
    }

    /// Drops the marks a run now answers, and all of them when the worker
    /// reported an error or a start. A notice names no item, so it clears
    /// every mark.
    pub(in crate::orchestrator::view) fn settle_starting(
        &mut self,
        notice: Option<&Notice>,
        read: bool,
    ) {
        let ended =
            notice.is_some_and(|notice| matches!(notice.outcome, Err(_) | Ok(Done::Started)));
        for repo in &mut self.repos {
            if ended {
                repo.starting.clear();
            } else if read && !repo.starting.is_empty() {
                let runs = &repo.snapshot.runs;
                repo.starting
                    .retain(|key| !runs.iter().any(|run| run.item_key == *key));
            }
        }
    }
}
