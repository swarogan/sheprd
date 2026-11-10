use super::*;
use crate::jira::{self, Error as JiraError};
use serde_json::{Value, json};

const BASE: &str = "https://acme.atlassian.net";

fn source() -> SourceKey {
    jira::source_key("acme.atlassian.net")
}

fn person(name: &str) -> Value {
    json!({ "accountId": "5b10ac8d82e05b22cc7d4ef5", "displayName": name, "active": true })
}

fn issue(key: &str) -> Value {
    json!({
        "expand": "operations,versionedRepresentations",
        "id": "10042",
        "self": "https://acme.atlassian.net/rest/api/3/issue/10042",
        "key": key,
        "fields": {
            "summary": format!("Summary of {key}"),
            "description": {
                "version": 1, "type": "doc",
                "content": [{ "type": "paragraph", "content": [
                    { "type": "text", "text": "Steps " },
                    { "type": "text", "text": "to", "marks": [{ "type": "strong" }] },
                    { "type": "text", "text": " reproduce" },
                ]}],
            },
            "status": {
                "name": "In Progress", "id": "3",
                "statusCategory": { "id": 4, "key": "indeterminate", "name": "In Progress" },
            },
            "priority": { "name": "High", "id": "2" },
            "issuetype": { "name": "Bug", "id": "10004" },
            "labels": ["backend", "urgent"],
            "assignee": person("Ada Lovelace"),
            "reporter": person("Grace Hopper"),
            "parent": { "id": "10001", "key": "PROJ-100", "fields": { "summary": "Epic" } },
            "created": "2026-10-01T10:00:00.000+0200",
            "updated": "2026-10-02T11:30:15.123+0000",
            "project": { "id": "10000", "key": "PROJ", "name": "Project X" },
        },
    })
}

fn page(issues: Vec<Value>, token: Option<&str>, last: bool) -> Value {
    let mut page = json!({ "issues": issues, "isLast": last });
    if let Some(token) = token {
        page["nextPageToken"] = json!(token);
    }
    page
}

/// What a fake fetch was asked: the address and the JSON body, `None` for a GET.
type Asked = std::rc::Rc<std::cell::RefCell<Vec<(String, Option<Value>)>>>;

/// Answers each request from `answers` in order, `/myself` when they run out.
fn replies(
    answers: Vec<Value>,
) -> (
    impl FnMut(&url::Url, Option<&Value>) -> Result<Vec<u8>>,
    Asked,
) {
    let asked = Asked::default();
    let log = asked.clone();
    let mut answers = answers.into_iter();
    let fetch = move |url: &url::Url, body: Option<&Value>| {
        log.borrow_mut().push((url.to_string(), body.cloned()));
        let answer = answers.next().unwrap_or_else(|| person("Ada Lovelace"));
        Ok(serde_json::to_vec(&answer).unwrap())
    };
    (fetch, asked)
}

fn keys(listing: &jira::Listing) -> Vec<&str> {
    listing
        .items
        .iter()
        .map(|item| item.key.native_id.as_str())
        .collect()
}

#[test]
fn the_search_sends_the_jql_the_fields_and_no_token_on_the_first_page() {
    let (mut fetch, asked) = replies(vec![page(vec![issue("PROJ-123")], None, true)]);
    jira::list(&source(), BASE, None, &mut fetch).unwrap();
    let asked = asked.borrow();
    assert_eq!(
        asked[0].0,
        "https://acme.atlassian.net/rest/api/3/search/jql"
    );
    assert_eq!(
        asked[0].1,
        Some(json!({
            "jql": "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC",
            "maxResults": 100,
            "fields": [
                "summary", "description", "status", "priority", "issuetype", "labels",
                "assignee", "reporter", "parent", "created", "updated", "project",
            ],
        }))
    );
    // The reader is asked for by a GET after the issues.
    assert_eq!(
        asked[1],
        (
            "https://acme.atlassian.net/rest/api/3/myself".to_owned(),
            None
        )
    );
    assert_eq!(asked.len(), 2);
}

#[test]
fn an_issue_is_listed_by_its_key_with_its_status_priority_parent_and_page() {
    let (mut fetch, _) = replies(vec![page(vec![issue("PROJ-123")], None, true)]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    assert_eq!(listing.viewer.as_deref(), Some("Ada Lovelace"));
    let [item] = &listing.items[..] else {
        panic!("{:?}", listing.items);
    };
    assert_eq!(
        item.key.canonical(),
        "jira:acme.atlassian.net:assigned:PROJ-123"
    );
    assert_eq!(item.identifier, "PROJ-123");
    assert_eq!(item.title, "Summary of PROJ-123");
    assert_eq!(item.description.as_deref(), Some("Steps **to** reproduce"));
    // The state is a word the lists know; Jira's own status leads the labels.
    assert_eq!(item.state, "in_progress");
    assert_eq!(
        item.labels,
        ["In Progress", "Bug", "Project X", "backend", "urgent"]
    );
    assert_eq!(item.priority, Some(1));
    assert_eq!(item.parent_id.as_deref(), Some("PROJ-100"));
    assert_eq!(
        item.url.as_deref(),
        Some("https://acme.atlassian.net/browse/PROJ-123")
    );
    assert_eq!(item.author.as_deref(), Some("Grace Hopper"));
    assert_eq!(item.assignee.as_deref(), Some("Ada Lovelace"));
    // Jira's offset has no colon; both forms read.
    assert_eq!(
        item.created_at.unwrap().to_rfc3339(),
        "2026-10-01T08:00:00+00:00"
    );
    assert_eq!(
        item.updated_at.unwrap().to_rfc3339(),
        "2026-10-02T11:30:15.123+00:00"
    );
    assert!(item.pull_request.is_none());
}

#[test]
fn status_categories_and_priorities_map_onto_the_states_and_numbers_lists_know() {
    let case = |category: Value, priority: Value| {
        let mut node = issue("PROJ-1");
        node["fields"]["status"] = json!({ "name": "Whatever", "statusCategory": category });
        node["fields"]["priority"] = priority;
        let (mut fetch, _) = replies(vec![page(vec![node], None, true)]);
        let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
        (listing.items[0].state.clone(), listing.items[0].priority)
    };
    for (category, state) in [
        ("new", "open"),
        ("indeterminate", "in_progress"),
        ("done", "closed"),
        ("something-new", "open"),
    ] {
        assert_eq!(
            case(json!({ "key": category }), json!(null)).0,
            state,
            "{category}"
        );
    }
    assert_eq!(case(Value::Null, Value::Null), ("open".to_owned(), None));
    for (name, priority) in [
        ("Highest", 0),
        ("high", 1),
        ("Medium", 2),
        ("Low", 3),
        ("Lowest", 4),
    ] {
        assert_eq!(
            case(Value::Null, json!({ "name": name })).1,
            Some(priority),
            "{name}"
        );
    }
    // A renamed priority falls back to the default scheme's id; a custom one has none.
    assert_eq!(
        case(Value::Null, json!({ "name": "Urgent!", "id": "1" })).1,
        Some(0)
    );
    assert_eq!(
        case(Value::Null, json!({ "name": "Urgent!", "id": "10050" })).1,
        None
    );
}

#[test]
fn an_issue_with_nothing_but_a_key_is_still_listed() {
    let (mut fetch, _) = replies(vec![page(
        vec![
            json!({ "key": "PROJ-9" }),
            json!({ "key": "PROJ-8", "fields": { "summary": "  ", "description": 7 } }),
        ],
        None,
        true,
    )]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert_eq!(keys(&listing), ["PROJ-9", "PROJ-8"]);
    for item in &listing.items {
        assert_eq!(item.title, item.identifier);
        assert_eq!(item.state, "open");
        assert!(item.description.is_none() && item.priority.is_none());
        assert!(item.labels.is_empty() && item.parent_id.is_none());
        assert!(item.created_at.is_none() && item.updated_at.is_none());
    }
}

#[test]
fn pages_follow_the_token_until_the_last() {
    let (mut fetch, asked) = replies(vec![
        page(vec![issue("PROJ-3"), issue("PROJ-2")], Some("tok-1"), false),
        page(vec![issue("PROJ-1")], Some("tok-2"), false),
        page(vec![issue("PROJ-0")], None, true),
    ]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    assert_eq!(keys(&listing), ["PROJ-3", "PROJ-2", "PROJ-1", "PROJ-0"]);
    let asked = asked.borrow();
    let tokens: Vec<_> = asked[..3]
        .iter()
        .map(|(_, body)| body.as_ref().unwrap().get("nextPageToken").cloned())
        .collect();
    assert_eq!(tokens, [None, Some(json!("tok-1")), Some(json!("tok-2"))]);
    // The three searches, then the reader.
    assert_eq!(asked.len(), 4);
}

#[test]
fn the_last_page_ends_the_listing_whatever_token_it_carries_and_no_token_does_too() {
    let (mut fetch, asked) = replies(vec![page(vec![issue("PROJ-1")], Some("stale"), true)]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    assert_eq!(asked.borrow().len(), 2);

    // A reply that says nothing of `isLast` and has no token is the end too.
    let (mut fetch, asked) = replies(vec![json!({ "issues": [issue("PROJ-1")] })]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    assert_eq!(asked.borrow().len(), 2);
}

fn full_page(from: usize, token: Option<&str>, last: bool) -> Value {
    page(
        (from..from + 100)
            .map(|n| issue(&format!("PROJ-{n}")))
            .collect(),
        token,
        last,
    )
}

#[test]
fn a_thousand_issues_with_more_to_come_are_a_capped_listing() {
    let tokens: Vec<String> = (0..10).map(|n| format!("tok-{n}")).collect();
    let answers = (0..10)
        .map(|n| full_page(n * 100, Some(&tokens[n]), false))
        .collect();
    let (mut fetch, asked) = replies(answers);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(!listing.complete);
    assert_eq!(listing.items.len(), jira::MAX_OPEN);
    // Ten searches and the reader; no eleventh page is asked for.
    assert_eq!(asked.borrow().len(), 11);
}

#[test]
fn a_thousand_issues_that_are_all_there_is_are_complete() {
    let answers = (0..10)
        .map(|n| {
            let last = n == 9;
            full_page(n * 100, (!last).then_some("more"), last)
        })
        .map(|mut page| {
            // Distinct tokens, as Jira gives.
            if page.get("nextPageToken").is_some() {
                page["nextPageToken"] = json!(format!("t{}", page["issues"][0]["key"]));
            }
            page
        })
        .collect();
    let (mut fetch, _) = replies(answers);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    assert_eq!(listing.items.len(), 1000);
}

#[test]
fn a_page_that_adds_nothing_or_repeats_a_token_ends_the_listing_incomplete() {
    // Only issues already read: asking again would never end.
    let (mut fetch, asked) = replies(vec![
        page(vec![issue("PROJ-1")], Some("a"), false),
        page(vec![issue("PROJ-1")], Some("b"), false),
    ]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(!listing.complete);
    assert_eq!(keys(&listing), ["PROJ-1"]);
    assert_eq!(asked.borrow().len(), 3);

    let (mut fetch, _) = replies(vec![
        page(vec![issue("PROJ-1")], Some("a"), false),
        page(vec![issue("PROJ-2")], Some("a"), false),
    ]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(!listing.complete);
    assert_eq!(keys(&listing), ["PROJ-1", "PROJ-2"]);
}

#[test]
fn a_refusal_or_a_reply_that_is_not_json_fails_the_listing_but_an_odd_reader_does_not() {
    let mut refused = |_: &url::Url, _: Option<&Value>| Err(JiraError::Authentication.into());
    assert!(matches!(
        jira::list(&source(), BASE, None, &mut refused),
        Err(Error::Jira(JiraError::Authentication))
    ));
    let mut garbage = |_: &url::Url, _: Option<&Value>| Ok(b"<html>".to_vec());
    assert!(matches!(
        jira::list(&source(), BASE, None, &mut garbage),
        Err(Error::Jira(JiraError::Json(_)))
    ));
    let (mut fetch, _) = replies(vec![page(vec![issue("PROJ-1")], None, true), json!([1, 2])]);
    let listing = jira::list(&source(), BASE, None, &mut fetch).unwrap();
    assert_eq!(listing.viewer, None);
    assert_eq!(listing.items.len(), 1);
}

#[test]
fn a_site_under_a_sub_path_keeps_it_and_the_key_is_escaped_in_the_link() {
    let (mut fetch, asked) = replies(vec![page(vec![issue("PROJ 1/x")], None, true)]);
    let listing = jira::list(&source(), "https://example.com/jira", None, &mut fetch).unwrap();
    assert_eq!(
        asked.borrow()[0].0,
        "https://example.com/jira/rest/api/3/search/jql"
    );
    assert_eq!(
        listing.items[0].url.as_deref(),
        Some("https://example.com/jira/browse/PROJ%201%2Fx")
    );
}

#[test]
fn the_checkpoint_keeps_the_markers_it_was_given() {
    let previous = Checkpoint {
        pr_cursor: Some(7),
        pr_details: [("a".to_owned(), "b".to_owned())].into(),
        ..Checkpoint::default()
    };
    let (mut fetch, _) = replies(vec![page(vec![], None, true)]);
    let listing = jira::list(&source(), BASE, Some(&previous), &mut fetch).unwrap();
    assert_eq!(listing.checkpoint.pr_cursor, Some(7));
    assert_eq!(listing.checkpoint.pr_details, previous.pr_details);
    assert!(listing.checkpoint.last_full_at.is_some());
}

#[test]
fn a_site_with_no_saved_sign_in_asks_to_connect_without_a_request() {
    let error = jira::sync(&source(), None, None, &|| false, &mut None).unwrap_err();
    let Error::Jira(error) = error else {
        panic!("{error}");
    };
    assert!(matches!(&error, JiraError::SignIn { site } if site == "acme.atlassian.net"));
    assert!(error.needs_sign_in() && error.needs_user());
}

#[test]
fn only_a_missing_or_refused_sign_in_offers_the_sign_in_page() {
    assert!(JiraError::Authentication.needs_sign_in());
    let not_found = JiraError::NotFound {
        origin: BASE.into(),
    };
    assert!(!not_found.needs_sign_in() && not_found.needs_user());
    for error in [
        JiraError::RateLimit,
        JiraError::Status(500),
        JiraError::Size,
    ] {
        assert!(!error.needs_sign_in() && !error.needs_user(), "{error}");
    }
}
