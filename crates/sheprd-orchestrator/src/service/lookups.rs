//! What the open page asks the worker to look up on demand: a pull request's
//! status, the transitions of a Jira issue, and an issue's comments.

use super::{
    Command, Error, Ordering, Provider, PullRequestLookup, Service, SourceKey, Worker, gitea, jira,
};
use crate::comments::{Comment, Lookup};
use std::sync::Arc;

impl Service {
    /// Asks for the transitions Jira offers `issue`; a full queue drops it.
    pub fn load_jira_transitions(&self, source: SourceKey, issue: String) {
        let _ = self
            .commands
            .try_send(Command::LoadJiraTransitions { source, issue });
    }

    /// Asks for the comments of `issue`; a full queue drops it.
    pub fn load_comments(&self, source: SourceKey, issue: String) {
        let _ = self
            .commands
            .try_send(Command::LoadComments { source, issue });
    }
}

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

    /// Reads the comments of `issue` with the sign-in saved for `source`'s
    /// instance or site, and shows them. Other providers have none to read.
    pub(super) fn load_comments(&mut self, source: &SourceKey, issue: &str) {
        let result: Result<Vec<Comment>, Error> = match source.provider {
            Provider::Gitea => {
                let saved = self.gitea_record();
                gitea::comments::comments(saved.get(&source.host), source, issue)
                    .map_err(Error::from)
            }
            Provider::Jira => {
                let saved = self.jira_record();
                match saved.get(&source.host) {
                    Some(site) => jira::comments::comments(site, issue).map_err(Error::from),
                    None => Err(jira::Error::SignIn {
                        site: source.host.clone(),
                    }
                    .into()),
                }
            }
            _ => return,
        };
        self.snapshot.comments = Some(Lookup {
            source: source.clone(),
            issue: issue.to_owned(),
            result: Arc::new(result),
        });
        self.publish();
    }
}
