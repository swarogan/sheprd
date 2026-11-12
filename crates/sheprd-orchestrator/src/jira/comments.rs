//! Reading the comments of a Jira issue for its page. Bodies are Atlassian
//! documents, turned into Markdown as the description is.

use super::{Error, Target, adf, agent, api, send, write::valid_issue};
use crate::comments::{Comment, capped, newest};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);
/// What a read of 100 comments may answer.
const LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    comments: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    author: Option<Author>,
    created: Option<String>,
    body: Option<Value>,
}

#[derive(Deserialize)]
struct Author {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

/// Jira's time, `2026-11-12T01:44:40.123+0000`, with a zone that has no
/// colon; a time in RFC 3339 form is read too.
fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f%z")
        .or_else(|_| DateTime::parse_from_rfc3339(text))
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// The newest comments of `issue` on the site in `target`, oldest first,
/// read from `api_base`. The newest are asked for first, so an issue with
/// more than a page of them keeps the latest.
pub(crate) fn comments_at(
    agent: &ureq::Agent,
    target: &Target<'_>,
    api_base: &str,
    issue: &str,
) -> Result<Vec<Comment>, Error> {
    let mut url = api(api_base, &["issue", valid_issue(issue)?, "comment"])?;
    url.query_pairs_mut()
        .append_pair("maxResults", &crate::comments::MAX_COMMENTS.to_string())
        .append_pair("orderBy", "-created");
    let bytes = send(agent, target, &url, None, LIMIT, &mut None)?;
    let page: Page = serde_json::from_slice(&bytes).map_err(Error::Json)?;
    let mut all: Vec<Comment> = page
        .comments
        .into_iter()
        .map(|node| Comment {
            author: node
                .author
                .and_then(|author| author.display_name)
                .unwrap_or_default(),
            created_at: node.created.as_deref().and_then(parse_time),
            body: capped(
                node.body
                    .as_ref()
                    .and_then(adf::to_markdown)
                    .unwrap_or_default(),
            ),
        })
        .collect();
    all.reverse();
    Ok(newest(all))
}

/// The comments of `issue` on `site`. Blocks on the network: call it from a
/// worker.
pub fn comments(site: &super::saved::Site, issue: &str) -> Result<Vec<Comment>, Error> {
    let target = Target {
        base_url: site.base_url(),
        email: site.email(),
        token: site.token(),
    };
    comments_at(&agent(TIMEOUT), &target, &site.api_base(), issue)
}
