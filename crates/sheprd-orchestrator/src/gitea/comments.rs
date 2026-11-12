//! Reading the comments of a Gitea issue or pull request for its page.
//! Owner, repository, and number are checked before a request is built.

use super::{
    Error, LIMIT, Repository, TIMEOUT, agent, endpoint, get,
    issue::User,
    saved::Instance,
    write::{valid_index, valid_name},
};
use crate::{
    SourceKey,
    comments::{Comment, capped, newest},
};
use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::Deserialize;

#[derive(Deserialize)]
struct Node {
    user: Option<User>,
    created_at: Option<String>,
    body: Option<String>,
}

/// The comments of issue or pull request `index` of `source`, which an
/// instance that is gone lacks. Blocks on the network: call it from a
/// worker.
pub fn comments(
    instance: Option<&Instance>,
    source: &SourceKey,
    index: &str,
) -> Result<Vec<Comment>, Error> {
    let Some(instance) = instance else {
        return Err(Error::SignIn {
            repository: source.repository.clone(),
        });
    };
    comments_at(
        &agent(TIMEOUT),
        instance.base_url(),
        instance.token(),
        source,
        index,
    )
}

/// [`comments`] with the call sent to `base_url` with `token`.
pub(crate) fn comments_at(
    agent: &ureq::Agent,
    base_url: &str,
    token: &SecretString,
    source: &SourceKey,
    index: &str,
) -> Result<Vec<Comment>, Error> {
    let repository = Repository::of(source, base_url)?;
    let (owner, name) = (valid_name(repository.owner)?, valid_name(repository.name)?);
    let index = valid_index(index)?;
    let url = endpoint(
        base_url,
        &["repos", owner, name, "issues", index, "comments"],
    )?;
    let bytes = get(
        agent,
        base_url,
        token,
        &url,
        &source.repository,
        LIMIT,
        &mut None,
    )?;
    let nodes: Vec<Node> = serde_json::from_slice(&bytes).map_err(Error::Json)?;
    let all = nodes
        .into_iter()
        .map(|node| Comment {
            author: node
                .user
                .as_ref()
                .and_then(User::login)
                .unwrap_or_default()
                .to_owned(),
            created_at: node
                .created_at
                .and_then(|at| DateTime::parse_from_rfc3339(&at).ok())
                .map(|at| at.with_timezone(&Utc)),
            body: capped(node.body.unwrap_or_default()),
        })
        .collect();
    Ok(newest(all))
}
