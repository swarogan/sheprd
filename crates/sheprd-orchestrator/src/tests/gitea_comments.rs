//! Reading a Gitea issue's comments for its page, against a local server
//! standing in for the instance: the path and token, the order and bounds,
//! and what is refused before any request is made.
use super::gitea_write::{authorization, serve, source, token};
use crate::{
    comments::{Comment, MAX_BODY, MAX_COMMENTS},
    gitea::{self, Error as GiteaError, comments::comments_at},
};
use chrono::{TimeZone, Utc};
use serde_json::{Value, json};
use std::time::Duration;

fn read(base: &str, index: &str) -> Result<Vec<Comment>, GiteaError> {
    read_of(base, &source(), index)
}

fn read_of(base: &str, source: &crate::SourceKey, index: &str) -> Result<Vec<Comment>, GiteaError> {
    comments_at(
        &gitea::agent(Duration::from_secs(5)),
        base,
        &token(),
        source,
        index,
    )
}

fn reply(comments: &[Value]) -> (u16, String) {
    (200, Value::Array(comments.to_vec()).to_string())
}

fn node(login: &str, at: &str, body: &str) -> Value {
    json!({ "user": { "login": login }, "created_at": at, "body": body })
}

#[test]
fn the_comments_are_read_from_the_issue_with_the_token_in_order() {
    let (base, server) = serve(vec![reply(&[
        node("me", "2026-11-10T08:00:00Z", "first **bold**"),
        node("ada", "2026-11-11T09:30:00+01:00", "second"),
    ])]);
    let found = read(&base, "7").unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("GET /api/v1/repos/acme/app/issues/7/comments "),
        "{requests:?}"
    );
    assert_eq!(authorization(&requests[0]), Some("token 0123456789abcdef"));
    assert_eq!(
        found,
        vec![
            Comment {
                author: "me".into(),
                created_at: Some(Utc.with_ymd_and_hms(2026, 11, 10, 8, 0, 0).unwrap()),
                body: "first **bold**".into(),
            },
            Comment {
                author: "ada".into(),
                created_at: Some(Utc.with_ymd_and_hms(2026, 11, 11, 8, 30, 0).unwrap()),
                body: "second".into(),
            },
        ]
    );
}

#[test]
fn a_comment_with_no_user_or_an_unreadable_time_still_lists() {
    let (base, server) = serve(vec![reply(&[
        json!({ "user": null, "created_at": "yesterday", "body": "hi" }),
    ])]);
    let found = read(&base, "7").unwrap();
    server.join().unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].author.as_str(), found[0].created_at), ("", None));
}

#[test]
fn more_than_a_hundred_comments_keep_the_newest_hundred_oldest_first() {
    let all: Vec<Value> = (0..MAX_COMMENTS + 5)
        .map(|n| node("me", "2026-11-10T08:00:00Z", &format!("c{n}")))
        .collect();
    let (base, server) = serve(vec![reply(&all)]);
    let found = read(&base, "7").unwrap();
    server.join().unwrap();
    assert_eq!(found.len(), MAX_COMMENTS);
    assert_eq!(found[0].body, "c5");
    assert_eq!(
        found[MAX_COMMENTS - 1].body,
        format!("c{}", MAX_COMMENTS + 4)
    );
}

#[test]
fn an_over_long_body_is_cut_on_a_character_boundary() {
    // Two-byte characters put the cap in the middle of one.
    let long = "é".repeat(MAX_BODY);
    let (base, server) = serve(vec![reply(&[node("me", "2026-11-10T08:00:00Z", &long)])]);
    let found = read(&base, "7").unwrap();
    server.join().unwrap();
    assert_eq!(found[0].body.len(), MAX_BODY);
    assert!(found[0].body.chars().all(|c| c == 'é'));
}

#[test]
fn a_pull_request_is_read_by_its_number_like_an_issue() {
    let (base, server) = serve(vec![reply(&[])]);
    assert!(read(&base, "12").unwrap().is_empty());
    let requests = server.join().unwrap();
    assert!(requests[0].starts_with("GET /api/v1/repos/acme/app/issues/12/comments "));
}

#[test]
fn an_invalid_owner_repository_or_number_is_refused_with_no_request() {
    // A server that would answer once; none is made, so it is never joined.
    let (base, _server) = serve(vec![reply(&[])]);
    for index in ["", "7/comments", "-1", "../7", "7?x=1"] {
        assert!(
            matches!(read(&base, index), Err(GiteaError::Issue)),
            "{index:?}"
        );
    }
    for repository in ["../app", "acme/..", "acme/a b", "acme/app/extra", "acme"] {
        let mut source = source();
        source.repository = repository.into();
        assert!(
            matches!(
                read_of(&base, &source, "7"),
                Err(GiteaError::Issue | GiteaError::Address(_))
            ),
            "{repository:?}"
        );
    }
}

#[test]
fn refusals_read_as_the_listing_reads_them() {
    for (status, expected) in [(401, "auth"), (403, "auth"), (404, "missing")] {
        let (base, server) = serve(vec![(status, "{}".into())]);
        let error = read(&base, "7").unwrap_err();
        server.join().unwrap();
        match (expected, error) {
            ("auth", GiteaError::Authentication) => {}
            ("missing", GiteaError::NotFound { repository }) => assert_eq!(repository, "acme/app"),
            (_, error) => panic!("{status}: {error:?}"),
        }
    }
}

#[test]
fn a_source_whose_instance_is_gone_asks_for_a_sign_in() {
    assert!(matches!(
        gitea::comments::comments(None, &source(), "7"),
        Err(GiteaError::SignIn { repository }) if repository == "acme/app"
    ));
}
