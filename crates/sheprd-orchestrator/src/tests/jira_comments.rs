//! Reading a Jira issue's comments for its page, against a local server
//! standing in for the site: the path and query, the Markdown of a body,
//! Jira's time form, and the gateway path.
use super::jira_account::{CLOUD_ID, header, ok, serve};
use super::jira_write::run;
use crate::{
    comments::{Comment, MAX_COMMENTS},
    jira::{self, Error as JiraError, comments::comments_at},
};
use chrono::{TimeZone, Utc};
use serde_json::{Value, json};

const BASIC: &str = "Basic bWVAZXhhbXBsZS5jb206czNjcmV0";

fn read(base: &str, issue: &str) -> Result<Vec<Comment>, JiraError> {
    run(base, |agent, target| {
        comments_at(agent, target, base, issue)
    })
}

fn node(author: Option<&str>, created: &str, body: Value) -> Value {
    let mut node = json!({ "created": created, "body": body });
    if let Some(name) = author {
        node["author"] = json!({ "displayName": name });
    }
    node
}

fn text_doc(text: &str) -> Value {
    json!({ "type": "doc", "version": 1, "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": text }] }
    ] })
}

#[test]
fn comments_are_read_with_the_query_and_listed_oldest_first() {
    // Asked newest first, so the list is turned around.
    let (base, server) = serve(vec![ok(json!({ "comments": [
        node(Some("Ada"), "2026-11-12T01:44:40.123+0000", text_doc("later")),
        node(Some("Bob"), "2026-11-11T10:00:00.000+0000", text_doc("earlier")),
    ] }))]);
    let found = read(&base, "PROJ-1").unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0]
            .starts_with("GET /rest/api/3/issue/PROJ-1/comment?maxResults=100&orderBy=-created "),
        "{requests:?}"
    );
    assert_eq!(
        header(&requests[0], "authorization").as_deref(),
        Some(BASIC)
    );
    let people: Vec<_> = found
        .iter()
        .map(|c| (c.author.as_str(), c.body.as_str()))
        .collect();
    assert_eq!(people, [("Bob", "earlier"), ("Ada", "later")]);
}

#[test]
fn jiras_time_form_without_a_colon_in_the_zone_is_the_right_instant() {
    let (base, server) = serve(vec![ok(json!({ "comments": [
        node(Some("Ada"), "2026-11-12T01:44:40.123+0100", text_doc("a")),
        node(Some("Ada"), "not a time", text_doc("b")),
    ] }))]);
    let found = read(&base, "PROJ-1").unwrap();
    server.join().unwrap();
    // Reversed: the unreadable one is first, and has no time.
    assert_eq!(found[0].created_at, None);
    let expected = Utc.with_ymd_and_hms(2026, 11, 12, 0, 44, 40).unwrap()
        + chrono::Duration::milliseconds(123);
    assert_eq!(found[1].created_at, Some(expected));
}

#[test]
fn an_atlassian_document_body_becomes_the_markdown_of_the_description() {
    let body = json!({ "type": "doc", "version": 1, "content": [
        { "type": "paragraph", "content": [
            { "type": "text", "text": "Done " },
            { "type": "text", "text": "really", "marks": [{ "type": "strong" }] } ] },
        { "type": "bulletList", "content": [
            { "type": "listItem", "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "one" }] }] },
            { "type": "listItem", "content": [
                { "type": "paragraph", "content": [{ "type": "text", "text": "two" }] }] } ] },
    ] });
    let (base, server) = serve(vec![ok(json!({ "comments": [
        node(Some("Ada"), "2026-11-12T01:44:40.123+0000", body.clone()),
    ] }))]);
    let found = read(&base, "PROJ-1").unwrap();
    server.join().unwrap();
    assert_eq!(Some(found[0].body.clone()), jira::adf::to_markdown(&body));
    assert!(found[0].body.contains("**really**"), "{}", found[0].body);
    assert!(found[0].body.contains("- one"), "{}", found[0].body);
}

#[test]
fn a_comment_with_no_author_lists_with_an_empty_name() {
    let (base, server) = serve(vec![ok(json!({ "comments": [
        node(None, "2026-11-12T01:44:40.123+0000", text_doc("anonymous")),
    ] }))]);
    let found = read(&base, "PROJ-1").unwrap();
    server.join().unwrap();
    assert_eq!(
        (found[0].author.as_str(), found[0].body.as_str()),
        ("", "anonymous")
    );
}

#[test]
fn more_than_a_hundred_comments_keep_the_newest_hundred() {
    // Newest first, as asked for: c0 is the newest.
    let all: Vec<Value> = (0..MAX_COMMENTS + 3)
        .map(|n| {
            node(
                Some("Ada"),
                "2026-11-12T01:44:40.123+0000",
                text_doc(&format!("c{n}")),
            )
        })
        .collect();
    let (base, server) = serve(vec![ok(json!({ "comments": all }))]);
    let found = read(&base, "PROJ-1").unwrap();
    server.join().unwrap();
    assert_eq!(found.len(), MAX_COMMENTS);
    assert_eq!(found[MAX_COMMENTS - 1].body, "c0");
    assert_eq!(found[0].body, format!("c{}", MAX_COMMENTS - 1));
}

#[test]
fn a_site_with_a_cloud_id_is_read_through_the_gateway_path() {
    let (origin, server) = serve(vec![ok(json!({ "comments": [] }))]);
    let base = jira::gateway_base(&origin, CLOUD_ID);
    let found = run(&origin, |agent, target| {
        comments_at(agent, target, &base, "PROJ-1")
    })
    .unwrap();
    assert!(found.is_empty());
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with(&format!(
            "GET /ex/jira/{CLOUD_ID}/rest/api/3/issue/PROJ-1/comment?"
        )),
        "{requests:?}"
    );
}

#[test]
fn an_invalid_key_is_refused_with_no_request() {
    let (base, _server) = serve(vec![ok(json!({ "comments": [] }))]);
    for key in ["", "PROJ-1/comment", "../PROJ-1", "PROJ-1?x=1"] {
        assert!(matches!(read(&base, key), Err(JiraError::Issue)), "{key:?}");
    }
}

#[test]
fn a_refusal_reads_as_the_other_reads_do() {
    for (status, expected) in [(401, "auth"), (403, "auth"), (404, "missing")] {
        let (base, server) = serve(vec![(status, Vec::new(), "{}".into())]);
        let error = read(&base, "PROJ-1").unwrap_err();
        server.join().unwrap();
        match (expected, error) {
            ("auth", JiraError::Authentication) | ("missing", JiraError::NotFound { .. }) => {}
            (_, error) => panic!("{status}: {error:?}"),
        }
    }
}
