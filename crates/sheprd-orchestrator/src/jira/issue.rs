//! A Jira issue in the shared issue shape, so the lists show it beside the
//! other providers'.

use crate::{Item, ItemKey, SourceKey};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

/// One reply of `POST /rest/api/3/search/jql`.
#[derive(Deserialize)]
pub struct Page {
    #[serde(default)]
    pub issues: Vec<Issue>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "isLast")]
    pub is_last: Option<bool>,
}

#[derive(Deserialize)]
struct Named {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Person {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

impl Person {
    fn name(self) -> Option<String> {
        self.display_name.filter(|name| !name.trim().is_empty())
    }
}

#[derive(Deserialize)]
struct Status {
    name: Option<String>,
    #[serde(rename = "statusCategory")]
    category: Option<Category>,
}

#[derive(Deserialize)]
struct Category {
    key: Option<String>,
}

#[derive(Deserialize)]
struct Parent {
    key: Option<String>,
}

#[derive(Default, Deserialize)]
struct Fields {
    summary: Option<String>,
    /// Atlassian Document Format; a plain string on an older site.
    description: Option<Value>,
    status: Option<Status>,
    priority: Option<Named>,
    #[serde(rename = "issuetype")]
    kind: Option<Named>,
    labels: Option<Vec<String>>,
    assignee: Option<Person>,
    reporter: Option<Person>,
    parent: Option<Parent>,
    created: Option<String>,
    updated: Option<String>,
    project: Option<Named>,
}

/// One entry of the search's `issues`.
#[derive(Deserialize)]
pub struct Issue {
    key: String,
    #[serde(default)]
    fields: Fields,
}

/// Jira writes `2026-10-01T10:00:00.000+0200`, which RFC 3339 does not
/// allow the offset of; a time neither form reads is left unknown.
fn time(text: Option<&str>) -> Option<DateTime<Utc>> {
    let text = text?;
    DateTime::parse_from_rfc3339(text)
        .or_else(|_| DateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

/// The state word the list knows for a status category: Jira's own status
/// name is kept as the first label.
fn state(category: Option<&str>) -> &'static str {
    match category {
        Some("indeterminate") => "in_progress",
        Some("done") => "closed",
        _ => "open",
    }
}

/// The priority as Beads numbers it, 0 the most urgent, from Jira's
/// default scheme by name, else by its id; a custom scheme has none.
fn priority(priority: &Named) -> Option<i64> {
    let by_name = match priority
        .name
        .as_deref()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "highest" | "blocker" | "critical" => Some(0),
        "high" | "major" => Some(1),
        "medium" => Some(2),
        "low" | "minor" => Some(3),
        "lowest" | "trivial" => Some(4),
        _ => None,
    };
    by_name.or_else(|| {
        let id: i64 = priority.id.as_deref()?.parse().ok()?;
        (1..=5).contains(&id).then_some(id - 1)
    })
}

impl Issue {
    pub fn key(&self) -> &str {
        &self.key
    }

    /// `base_url` is the site's address, which the issue's page hangs from.
    pub fn into_item(self, source: &SourceKey, base_url: &str) -> Item {
        let fields = self.fields;
        let category = fields
            .status
            .as_ref()
            .and_then(|status| status.category.as_ref()?.key.as_deref());
        let state = state(category).to_owned();
        let priority = fields.priority.as_ref().and_then(priority);
        // The status's own name and the kind lead the labels, as Azure's
        // work items do, since `state` takes only the words the lists know.
        let labels = fields
            .status
            .and_then(|status| status.name)
            .into_iter()
            .chain(fields.kind.and_then(|kind| kind.name))
            .chain(fields.project.and_then(|project| project.name))
            .chain(fields.labels.unwrap_or_default())
            .filter(|label| !label.trim().is_empty())
            .collect();
        let url = super::endpoint(base_url, &["browse", &self.key])
            .ok()
            .map(String::from);
        let created_at = time(fields.created.as_deref());
        Item {
            key: ItemKey {
                source: source.clone(),
                native_id: self.key.clone(),
            },
            title: fields
                .summary
                .filter(|summary| !summary.trim().is_empty())
                .unwrap_or_else(|| self.key.clone()),
            identifier: self.key,
            description: fields
                .description
                .as_ref()
                .and_then(super::adf::to_markdown),
            acceptance: None,
            state,
            url,
            author: fields.reporter.and_then(Person::name),
            assignee: fields.assignee.and_then(Person::name),
            labels,
            parent_id: fields
                .parent
                .and_then(|parent| parent.key)
                .filter(|key| !key.is_empty()),
            blocked_by: Vec::new(),
            priority,
            created_at,
            updated_at: time(fields.updated.as_deref()).or(created_at),
            pull_request: None,
            activity: None,
        }
    }
}
