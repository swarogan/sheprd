//! What the open page asks the worker to look up on demand: a pull request's
//! status, and the transitions of a Jira issue.

use super::{Ordering, Provider, PullRequestLookup, SourceKey, Worker, jira};
use std::sync::Arc;

impl Worker {
    pub(super) fn load_pull_request(&mut self, number: u64) {
        let remote = self
            .snapshot
            .repo
            .as_ref()
            .and_then(|info| info.remote.as_ref())
            .filter(|remote| remote.source.provider == Provider::Github);
        let (Some(remote), Some(token)) = (remote, self.request.token.clone()) else {
            return;
        };
        let Some((owner, repo)) = remote.source.repository.split_once('/') else {
            return;
        };
        let cancelled = self.cancelled.clone();
        let result = sheprd_github::pull_request::by_number(
            owner,
            repo,
            number,
            token.for_owner(owner),
            || cancelled.load(Ordering::Relaxed),
            &mut self.cooldown,
        );
        self.snapshot.pull_request = Some(PullRequestLookup {
            number,
            result: Arc::new(result),
        });
        self.publish();
    }

    /// Reads the transitions Jira offers `issue`, with the sign-in saved for
    /// `source`'s site, and shows them.
    pub(super) fn load_jira_transitions(&mut self, source: &SourceKey, issue: &str) {
        let saved = self.jira_record();
        let result = match saved.get(&source.host) {
            Some(site) => jira::write::transitions(site, issue),
            None => Err(jira::Error::SignIn {
                site: source.host.clone(),
            }),
        };
        self.snapshot.jira_transitions = Some(jira::Lookup {
            source: source.clone(),
            issue: issue.to_owned(),
            result: Arc::new(result),
        });
        self.publish();
    }
}
