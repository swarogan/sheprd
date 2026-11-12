//! The comments on an issue or pull request, in one shape for every
//! provider, and the bounds a page keeps them within.

use crate::{Error, SourceKey};
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// The most comments one page keeps: the newest.
pub const MAX_COMMENTS: usize = 100;
/// The longest comment body kept, in bytes.
pub const MAX_BODY: usize = 64 * 1024;

/// One comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    /// The author's name as the provider shows it; empty when it names none.
    pub author: String,
    pub created_at: Option<DateTime<Utc>>,
    /// Markdown.
    pub body: String,
}

/// Comments loaded for one issue, or why they could not be.
#[derive(Clone, Debug)]
pub struct Lookup {
    pub source: SourceKey,
    /// The id the page asked with: a Gitea issue's number, a Jira key.
    pub issue: String,
    pub result: Arc<Result<Vec<Comment>, Error>>,
}

/// `body` cut to [`MAX_BODY`] bytes on a character boundary.
pub(crate) fn capped(mut body: String) -> String {
    let mut end = MAX_BODY.min(body.len());
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body.truncate(end);
    body
}

/// The newest [`MAX_COMMENTS`] of `comments`, which are oldest first.
pub(crate) fn newest(mut comments: Vec<Comment>) -> Vec<Comment> {
    let extra = comments.len().saturating_sub(MAX_COMMENTS);
    comments.drain(..extra);
    comments
}
