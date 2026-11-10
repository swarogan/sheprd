//! A Gitea issue in the shared issue shape, so the lists show it beside the
//! other providers'.

use crate::{Activity, Item, ItemKey, SourceKey};
use chrono::{DateTime, Utc};
use serde::{Deserialize, de::IgnoredAny};

/// A person as Gitea names one.
#[derive(Deserialize)]
pub struct User {
    login: Option<String>,
    username: Option<String>,
}

impl User {
    /// The sign-in name, which `/user` names the reader by as well.
    pub fn login(&self) -> Option<&str> {
        self.login
            .as_deref()
            .or(self.username.as_deref())
            .filter(|name| !name.is_empty())
    }
}

#[derive(Deserialize)]
pub struct Label {
    pub name: String,
}

/// One entry of `GET /repos/{owner}/{repo}/issues`.
#[derive(Deserialize)]
pub struct IssueNode {
    number: u64,
    title: String,
    body: Option<String>,
    user: Option<User>,
    labels: Option<Vec<Label>>,
    comments: Option<u64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// Set when the entry is a pull request, which Gitea also lists.
    pull_request: Option<IgnoredAny>,
}

impl IssueNode {
    pub fn number(&self) -> u64 {
        self.number
    }

    pub fn is_pull_request(&self) -> bool {
        self.pull_request.is_some()
    }

    /// `web` is the repository's address, which the issue's page hangs from.
    pub fn into_item(self, source: &SourceKey, web: &str) -> Item {
        Item {
            key: ItemKey {
                source: source.clone(),
                native_id: self.number.to_string(),
            },
            identifier: format!("#{}", self.number),
            title: self.title,
            description: self.body.filter(|body| !body.is_empty()),
            // The listing asks for open issues only.
            state: "open".to_owned(),
            url: Some(format!("{web}/issues/{}", self.number)),
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
            pull_request: None,
            activity: Some(Activity {
                comments: self.comments,
                ..Activity::default()
            }),
        }
    }
}
