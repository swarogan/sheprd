//! What a person can change on a Jira issue from its page: its status, a
//! comment, and who it is assigned to. Every path segment is checked before
//! a request is built, and a refused write has its own failure.

use super::{Error, Target, account, agent, api, authorization, send, status_error};
use crate::SourceKey;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

/// The most transitions one issue's page keeps.
pub const MAX_TRANSITIONS: usize = 50;
/// The longest comment, in bytes.
pub const MAX_COMMENT: usize = 32 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
/// What a read of transitions or of the token's owner may answer.
const LIMIT: u64 = 1024 * 1024;
/// The longest issue key Jira's own limits leave room for.
const MAX_KEY: usize = 255;
const MAX_ID: usize = 20;

/// A move to another status that Jira offers an issue now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub id: String,
    pub name: String,
    /// The status the issue ends up in.
    pub to: Option<String>,
}

/// Transitions loaded for one issue, or why they could not be.
#[derive(Clone, Debug)]
pub struct Lookup {
    pub source: SourceKey,
    pub issue: String,
    pub result: Arc<Result<Vec<Transition>, Error>>,
}

/// One write to an issue of `source`'s site.
#[derive(Clone, Debug)]
pub enum Change {
    Transition {
        source: SourceKey,
        issue: String,
        id: String,
    },
    Comment {
        source: SourceKey,
        issue: String,
        text: String,
    },
    Assign {
        source: SourceKey,
        issue: String,
    },
}

impl Change {
    /// The source whose site is written to.
    pub fn source(&self) -> &SourceKey {
        match self {
            Self::Transition { source, .. }
            | Self::Comment { source, .. }
            | Self::Assign { source, .. } => source,
        }
    }
}

#[derive(Deserialize)]
struct Transitions {
    transitions: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    name: String,
    to: Option<Status>,
}

#[derive(Deserialize)]
struct Status {
    name: String,
}

/// `key`, when it can be one path segment of an issue.
pub(crate) fn valid_issue(key: &str) -> Result<&str, Error> {
    let fits = !key.is_empty()
        && key.len() <= MAX_KEY
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if fits { Ok(key) } else { Err(Error::Issue) }
}

/// `id`, when it is a transition's number.
pub(crate) fn valid_transition(id: &str) -> Result<&str, Error> {
    let fits = !id.is_empty() && id.len() <= MAX_ID && id.bytes().all(|byte| byte.is_ascii_digit());
    if fits { Ok(id) } else { Err(Error::Transition) }
}

/// The Atlassian document of `text`: a paragraph per line, an empty one for
/// an empty line.
pub fn comment_body(text: &str) -> Result<Value, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Comment);
    }
    if text.len() > MAX_COMMENT {
        return Err(Error::CommentLength { limit: MAX_COMMENT });
    }
    let content: Vec<Value> = text
        .lines()
        .map(|line| {
            if line.is_empty() {
                json!({ "type": "paragraph" })
            } else {
                json!({ "type": "paragraph", "content": [{ "type": "text", "text": line }] })
            }
        })
        .collect();
    Ok(json!({ "type": "doc", "version": 1, "content": content }))
}

/// The transitions of `issue` on the site in `target`, at most
/// [`MAX_TRANSITIONS`], read from `api_base`.
pub(crate) fn transitions_at(
    agent: &ureq::Agent,
    target: &Target<'_>,
    api_base: &str,
    issue: &str,
) -> Result<Vec<Transition>, Error> {
    let url = api(api_base, &["issue", valid_issue(issue)?, "transitions"])?;
    let bytes = send(agent, target, &url, None, LIMIT, &mut None)?;
    let reply: Transitions = serde_json::from_slice(&bytes).map_err(Error::Json)?;
    Ok(reply
        .transitions
        .into_iter()
        .take(MAX_TRANSITIONS)
        .map(|node| Transition {
            id: node.id,
            name: node.name,
            to: node.to.map(|status| status.name),
        })
        .collect())
}

/// The transitions of `issue` on `site`. Blocks on the network: call it from
/// a worker.
pub fn transitions(site: &super::saved::Site, issue: &str) -> Result<Vec<Transition>, Error> {
    let (agent, target) = (agent(TIMEOUT), target(site));
    transitions_at(&agent, &target, &site.api_base(), issue)
}

fn target(site: &super::saved::Site) -> Target<'_> {
    Target {
        base_url: site.base_url(),
        email: site.email(),
        token: site.token(),
    }
}

/// Makes `change` on `site`, which a source whose site is gone lacks.
/// Blocks on the network: call it from a worker.
pub fn perform(site: Option<&super::saved::Site>, change: &Change) -> Result<(), Error> {
    let Some(site) = site else {
        return Err(Error::SignIn {
            site: change.source().host.clone(),
        });
    };
    perform_at(&agent(TIMEOUT), &target(site), &site.api_base(), change)
}

/// [`perform`] with the calls sent to `api_base`.
pub(crate) fn perform_at(
    agent: &ureq::Agent,
    target: &Target<'_>,
    api_base: &str,
    change: &Change,
) -> Result<(), Error> {
    match change {
        Change::Transition { issue, id, .. } => {
            let (issue, id) = (valid_issue(issue)?, valid_transition(id)?);
            let url = api(api_base, &["issue", issue, "transitions"])?;
            let body = json!({ "transition": { "id": id } });
            write(agent, target, Verb::Post, &url, &body, 204)
        }
        Change::Comment { issue, text, .. } => {
            let issue = valid_issue(issue)?;
            let body = json!({ "body": comment_body(text)? });
            let url = api(api_base, &["issue", issue, "comment"])?;
            write(agent, target, Verb::Post, &url, &body, 201)
        }
        Change::Assign { issue, .. } => {
            let issue = valid_issue(issue)?;
            let me = send(
                agent,
                target,
                &api(api_base, &["myself"])?,
                None,
                LIMIT,
                &mut None,
            )?;
            let me: account::Myself = serde_json::from_slice(&me).map_err(Error::Json)?;
            let id = me
                .account_id()
                .filter(|id| !id.is_empty())
                .ok_or(Error::Authentication)?;
            let url = api(api_base, &["issue", issue, "assignee"])?;
            write(
                agent,
                target,
                Verb::Put,
                &url,
                &json!({ "accountId": id }),
                204,
            )
        }
    }
}

#[derive(Clone, Copy)]
enum Verb {
    Post,
    Put,
}

/// Sends `body` to `url` and wants exactly `expected` back, whose body is
/// ignored. Jira refusing the sign-in's right to write is [`Error::Forbidden`].
fn write(
    agent: &ureq::Agent,
    target: &Target<'_>,
    verb: Verb,
    url: &url::Url,
    body: &Value,
    expected: u16,
) -> Result<(), Error> {
    let authorization = authorization(target.email, target.token)?;
    let bytes = serde_json::to_vec(body).map_err(Error::Json)?;
    let request = match verb {
        Verb::Post => agent.post(url.as_str()),
        Verb::Put => agent.put(url.as_str()),
    };
    let reply = request
        .header("User-Agent", "Sheprd")
        .header("Accept", "application/json")
        .header("Authorization", authorization)
        .header("Content-Type", "application/json")
        .send(&bytes[..])
        .map_err(|error| Error::Network {
            origin: target.base_url.to_owned(),
            source: Box::new(error),
        })?;
    let status = reply.status().as_u16();
    if status == expected {
        return Ok(());
    }
    tracing::warn!(
        category = "orchestrator_jira",
        status,
        "Jira refused a change"
    );
    Err(match status {
        401 | 403 => Error::Forbidden,
        404 => Error::Status(404),
        status => status_error(status, target.base_url),
    })
}
