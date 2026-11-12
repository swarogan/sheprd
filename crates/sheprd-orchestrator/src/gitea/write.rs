//! What a person can change on a Gitea issue from its page: close or reopen
//! it, comment, and take it. Owner, repository, and number are checked before
//! a request is built, and a refused write has its own failure.

use super::{
    Error, LIMIT, Repository, TIMEOUT, agent, endpoint, get, issue::User, saved::Instance,
    status_error,
};
use crate::SourceKey;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};

/// The longest comment, in bytes.
pub const MAX_COMMENT: usize = 32 * 1024;
/// The longest owner or repository name.
const MAX_NAME: usize = 255;
const MAX_INDEX: usize = 20;

/// One write to an issue of `source`'s instance. A pull request takes only
/// a comment.
#[derive(Clone, Debug)]
pub enum Change {
    SetState {
        source: SourceKey,
        index: String,
        closed: bool,
    },
    Comment {
        source: SourceKey,
        index: String,
        text: String,
    },
    Assign {
        source: SourceKey,
        index: String,
    },
}

impl Change {
    /// The source whose instance is written to.
    pub fn source(&self) -> &SourceKey {
        match self {
            Self::SetState { source, .. }
            | Self::Comment { source, .. }
            | Self::Assign { source, .. } => source,
        }
    }

    fn index(&self) -> &str {
        match self {
            Self::SetState { index, .. }
            | Self::Comment { index, .. }
            | Self::Assign { index, .. } => index,
        }
    }
}

/// `name`, when it can be one path segment of an owner or a repository.
fn valid_name(name: &str) -> Result<&str, Error> {
    let fits = !name.is_empty()
        && name.len() <= MAX_NAME
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if fits { Ok(name) } else { Err(Error::Issue) }
}

/// `index`, when it is an issue's number.
fn valid_index(index: &str) -> Result<&str, Error> {
    let fits = !index.is_empty()
        && index.len() <= MAX_INDEX
        && index.bytes().all(|byte| byte.is_ascii_digit());
    if fits { Ok(index) } else { Err(Error::Issue) }
}

/// `text` trimmed, when it is a postable comment.
fn comment_text(text: &str) -> Result<&str, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Comment);
    }
    if text.len() > MAX_COMMENT {
        return Err(Error::CommentLength { limit: MAX_COMMENT });
    }
    Ok(text)
}

#[derive(Clone, Copy)]
enum Verb {
    Post,
    Patch,
}

/// Makes `change` on `instance`, which a source whose instance is gone
/// lacks. Blocks on the network: call it from a worker.
pub fn perform(instance: Option<&Instance>, change: &Change) -> Result<(), Error> {
    let Some(instance) = instance else {
        return Err(Error::SignIn {
            repository: change.source().repository.clone(),
        });
    };
    perform_at(
        &agent(TIMEOUT),
        instance.base_url(),
        instance.token(),
        change,
    )
}

/// [`perform`] with the calls sent to `base_url` with `token`.
pub(crate) fn perform_at(
    agent: &ureq::Agent,
    base_url: &str,
    token: &SecretString,
    change: &Change,
) -> Result<(), Error> {
    let repository = Repository::of(change.source(), base_url)?;
    let (owner, name) = (valid_name(repository.owner)?, valid_name(repository.name)?);
    let index = valid_index(change.index())?;
    let issue = |tail: &[&str]| {
        let mut path = vec!["repos", owner, name, "issues", index];
        path.extend(tail);
        endpoint(base_url, &path)
    };
    match change {
        Change::SetState { closed, .. } => {
            let state = if *closed { "closed" } else { "open" };
            let body = json!({ "state": state });
            write(agent, base_url, token, Verb::Patch, &issue(&[])?, &body)
        }
        Change::Comment { text, .. } => {
            let body = json!({ "body": comment_text(text)? });
            let url = issue(&["comments"])?;
            write(agent, base_url, token, Verb::Post, &url, &body)
        }
        Change::Assign { .. } => {
            let me = get(
                agent,
                base_url,
                token,
                &endpoint(base_url, &["user"])?,
                &change.source().repository,
                LIMIT,
                &mut None,
            )?;
            let me: User = serde_json::from_slice(&me).map_err(Error::Json)?;
            let login = me.login().ok_or(Error::Authentication)?;
            let body = json!({ "assignees": [login] });
            write(agent, base_url, token, Verb::Patch, &issue(&[])?, &body)
        }
    }
}

/// Sends `body` to `url` and wants exactly 201 back, whose body is ignored.
/// Gitea refusing the sign-in's right to write is [`Error::Forbidden`].
fn write(
    agent: &ureq::Agent,
    base_url: &str,
    token: &SecretString,
    verb: Verb,
    url: &url::Url,
    body: &Value,
) -> Result<(), Error> {
    let mut authorization = ureq::http::HeaderValue::from_str(&zeroize::Zeroizing::new(format!(
        "token {}",
        token.expose_secret()
    )))
    .map_err(|_| Error::Token)?;
    authorization.set_sensitive(true);
    let bytes = serde_json::to_vec(body).map_err(Error::Json)?;
    let request = match verb {
        Verb::Post => agent.post(url.as_str()),
        Verb::Patch => agent.patch(url.as_str()),
    };
    let reply = request
        .header("User-Agent", "Sheprd")
        .header("Accept", "application/json")
        .header("Authorization", authorization)
        .header("Content-Type", "application/json")
        .send(&bytes[..])
        .map_err(|error| Error::Network {
            origin: base_url.to_owned(),
            source: Box::new(error),
        })?;
    let status = reply.status().as_u16();
    if status == 201 {
        return Ok(());
    }
    tracing::warn!(
        category = "orchestrator_gitea",
        status,
        "Gitea refused a change"
    );
    Err(match status {
        401 | 403 => Error::Forbidden,
        status => status_error(status, base_url),
    })
}
