//! A Gitea pull request in the shared pull request shape.

use super::issue::{Label, User};
use crate::{Activity, Item, ItemKey, PullRequest, SourceKey};
use chrono::{DateTime, Utc};
use serde::Deserialize;

/// One entry of `GET /repos/{owner}/{repo}/pulls`.
#[derive(Deserialize)]
pub struct PullRequestNode {
    number: u64,
    title: String,
    body: Option<String>,
    /// `open` or `closed`; a merged one is `closed` with `merged` set.
    state: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged: bool,
    user: Option<User>,
    labels: Option<Vec<Label>>,
    comments: Option<u64>,
    additions: Option<u64>,
    deletions: Option<u64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    head: Branch,
    base: Branch,
}

#[derive(Deserialize)]
struct Branch {
    #[serde(rename = "ref")]
    name: String,
    #[serde(default)]
    sha: String,
    /// Gone when the fork was deleted.
    repo: Option<RepositoryRef>,
}

#[derive(Deserialize)]
struct RepositoryRef {
    full_name: String,
}

impl PullRequestNode {
    pub fn number(&self) -> u64 {
        self.number
    }

    /// Who opened it, by sign-in name.
    pub fn author(&self) -> Option<&str> {
        self.user.as_ref()?.login()
    }

    /// The state word the list filters and colors by.
    fn state(&self) -> &'static str {
        if self.merged {
            "merged"
        } else if self.state == "closed" {
            "closed"
        } else if self.draft {
            "draft"
        } else {
            "open"
        }
    }

    /// `web` is the repository's address, which the pull request's page
    /// hangs from.
    pub fn into_item(self, source: &SourceKey, web: &str) -> Item {
        let state = self.state();
        let number = self.number;
        Item {
            key: ItemKey {
                source: source.clone(),
                native_id: format!("pr/{number}"),
            },
            identifier: format!("#{number}"),
            title: self.title,
            description: self.body.filter(|body| !body.is_empty()),
            state: state.to_owned(),
            url: Some(format!("{web}/pulls/{number}")),
            author: self.user.as_ref().and_then(User::login).map(str::to_owned),
            labels: self
                .labels
                .unwrap_or_default()
                .into_iter()
                .map(|label| label.name)
                .collect(),
            parent_id: None,
            blocked_by: Vec::new(),
            assignee: None,
            acceptance: None,
            priority: None,
            created_at: Some(self.created_at),
            updated_at: Some(self.updated_at),
            pull_request: Some(PullRequest {
                number,
                additions: self.additions,
                deletions: self.deletions,
                base_ref: self.base.name,
                head_ref: self.head.name,
                base_sha: self.base.sha,
                head_sha: self.head.sha,
                head_repository: self.head.repo.map(|repo| repo.full_name),
            }),
            activity: Some(Activity {
                comments: self.comments,
                ..Activity::default()
            }),
        }
    }
}
