//! Writing to a Jira issue from its page, against a local server standing in
//! for the site: the paths, the bodies, the statuses a write wants, and what
//! is refused before any request is made.
use super::jira_account::{Answer, CLOUD_ID, header, ok, serve, token};
use crate::{
    SourceKey,
    jira::{
        self, Change, Error as JiraError, Target,
        write::{MAX_COMMENT, MAX_TRANSITIONS, comment_body, perform_at, transitions_at},
    },
};
use serde_json::{Value, json};
use std::time::Duration;

const BASIC: &str = "Basic bWVAZXhhbXBsZS5jb206czNjcmV0";

fn source() -> SourceKey {
    jira::source_key("acme.atlassian.net")
}

fn run<T>(base: &str, call: impl FnOnce(&ureq::Agent, &Target<'_>) -> T) -> T {
    let token = token();
    let target = Target {
        base_url: base,
        email: "me@example.com",
        token: &token,
    };
    call(&jira::agent(Duration::from_secs(5)), &target)
}

fn change(base: &str, change: &Change) -> Result<(), JiraError> {
    run(base, |agent, target| {
        perform_at(agent, target, base, change)
    })
}

fn transition(issue: &str, id: &str) -> Change {
    Change::Transition {
        source: source(),
        issue: issue.into(),
        id: id.into(),
    }
}

fn comment(text: &str) -> Change {
    Change::Comment {
        source: source(),
        issue: "PROJ-1".into(),
        text: text.into(),
    }
}

fn assign(issue: &str) -> Change {
    Change::Assign {
        source: source(),
        issue: issue.into(),
    }
}

fn empty(status: u16) -> Answer {
    (status, Vec::new(), String::new())
}

fn body_of(request: &str) -> Value {
    let (_, body) = request.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap()
}

#[test]
fn the_transitions_of_an_issue_are_listed_with_their_target_status() {
    let reply = json!({ "transitions": [
        { "id": "31", "name": "Start progress",
          "to": { "name": "In Progress", "statusCategory": { "key": "indeterminate" } } },
        { "id": "41", "name": "Close" },
    ] });
    let (base, server) = serve(vec![ok(reply)]);
    let found = run(&base, |agent, target| {
        transitions_at(agent, target, &base, "PROJ-1")
    })
    .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|t| (t.id.as_str(), t.name.as_str(), t.to.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("31", "Start progress", Some("In Progress")),
            ("41", "Close", None)
        ]
    );
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("GET /rest/api/3/issue/PROJ-1/transitions "),
        "{requests:?}"
    );
    assert_eq!(
        header(&requests[0], "authorization").as_deref(),
        Some(BASIC)
    );
}

#[test]
fn a_longer_list_of_transitions_is_cut() {
    let many: Vec<Value> = (0..MAX_TRANSITIONS + 20)
        .map(|n| json!({ "id": n.to_string(), "name": "Move" }))
        .collect();
    let (base, server) = serve(vec![ok(json!({ "transitions": many }))]);
    let found = run(&base, |agent, target| {
        transitions_at(agent, target, &base, "PROJ-1")
    })
    .unwrap();
    assert_eq!(found.len(), MAX_TRANSITIONS);
    server.join().unwrap();
}

#[test]
fn a_transition_posts_its_id_and_wants_204() {
    let (base, server) = serve(vec![empty(204)]);
    change(&base, &transition("PROJ-1", "31")).unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("POST /rest/api/3/issue/PROJ-1/transitions "),
        "{requests:?}"
    );
    assert_eq!(
        body_of(&requests[0]),
        json!({ "transition": { "id": "31" } })
    );
    assert_eq!(
        header(&requests[0], "authorization").as_deref(),
        Some(BASIC)
    );
}

#[test]
fn a_write_answered_with_another_success_status_is_not_taken_for_done() {
    let (base, server) = serve(vec![(200, Vec::new(), "{}".into())]);
    assert!(matches!(
        change(&base, &transition("PROJ-1", "31")),
        Err(JiraError::Status(200))
    ));
    server.join().unwrap();
}

#[test]
fn a_comment_posts_a_paragraph_per_line_and_wants_201() {
    let (base, server) = serve(vec![empty(201)]);
    change(&base, &comment("  first\n\nsecond  ")).unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("POST /rest/api/3/issue/PROJ-1/comment "),
        "{requests:?}"
    );
    assert_eq!(
        body_of(&requests[0]),
        json!({ "body": { "type": "doc", "version": 1, "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "first" }] },
            { "type": "paragraph" },
            { "type": "paragraph", "content": [{ "type": "text", "text": "second" }] },
        ] } })
    );
}

#[test]
fn an_empty_or_oversized_comment_is_refused_without_a_request() {
    let (base, server) = serve(Vec::new());
    for text in ["", "  \n\t "] {
        assert!(matches!(
            change(&base, &comment(text)),
            Err(JiraError::Comment)
        ));
    }
    let long = "x".repeat(MAX_COMMENT + 1);
    assert!(matches!(
        comment_body(&long),
        Err(JiraError::CommentLength { limit }) if limit == MAX_COMMENT
    ));
    assert!(comment_body(&"x".repeat(MAX_COMMENT)).is_ok());
    assert!(server.join().unwrap().is_empty());
}

#[test]
fn assigning_to_me_reads_the_account_and_puts_its_id() {
    let (base, server) = serve(vec![
        ok(json!({ "accountId": "5b10ac8d82e05b22cc7d4ef5", "displayName": "Ada" })),
        empty(204),
    ]);
    change(&base, &assign("PROJ-1")).unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with("GET /rest/api/3/myself "),
        "{requests:?}"
    );
    assert!(
        requests[1].starts_with("PUT /rest/api/3/issue/PROJ-1/assignee "),
        "{requests:?}"
    );
    assert_eq!(
        body_of(&requests[1]),
        json!({ "accountId": "5b10ac8d82e05b22cc7d4ef5" })
    );
}

#[test]
fn a_write_refused_with_401_or_403_is_its_own_failure() {
    for status in [401, 403] {
        let (base, server) = serve(vec![empty(status)]);
        let error = change(&base, &comment("hi")).unwrap_err();
        assert!(matches!(error, JiraError::Forbidden), "{status}: {error:?}");
        assert_eq!(
            error.to_string(),
            "Jira refused the change; a token with scopes needs the write:jira-work scope, and your account needs permission on the issue"
        );
        server.join().unwrap();
    }
    let (base, server) = serve(vec![empty(404)]);
    assert!(matches!(
        change(&base, &transition("PROJ-1", "31")),
        Err(JiraError::Status(404))
    ));
    server.join().unwrap();
}

#[test]
fn an_unusable_issue_key_or_transition_id_is_refused_without_a_request() {
    let (base, server) = serve(Vec::new());
    for key in ["../x", "", "a/b", "PROJ 1", "PROJ-1?x=1", &"A".repeat(256)] {
        for kind in [transition(key, "31"), comment_for(key), assign(key)] {
            assert!(
                matches!(change(&base, &kind), Err(JiraError::Issue)),
                "{key:?}"
            );
        }
        assert!(matches!(
            run(&base, |agent, target| transitions_at(
                agent, target, &base, key
            )),
            Err(JiraError::Issue)
        ));
    }
    for id in ["3a", "", "-1", "1/2", &"1".repeat(21)] {
        assert!(matches!(
            change(&base, &transition("PROJ-1", id)),
            Err(JiraError::Transition)
        ));
    }
    assert!(server.join().unwrap().is_empty());
}

fn comment_for(issue: &str) -> Change {
    Change::Comment {
        source: source(),
        issue: issue.into(),
        text: "hi".into(),
    }
}

#[test]
fn a_site_with_a_cloud_id_is_written_through_the_gateway_path() {
    let (origin, server) = serve(vec![empty(204)]);
    let base = jira::gateway_base(&origin, CLOUD_ID);
    run(&origin, |agent, target| {
        perform_at(agent, target, &base, &transition("PROJ-1", "31"))
    })
    .unwrap();
    let requests = server.join().unwrap();
    assert!(
        requests[0].starts_with(&format!(
            "POST /ex/jira/{CLOUD_ID}/rest/api/3/issue/PROJ-1/transitions "
        )),
        "{requests:?}"
    );
}

#[test]
fn a_source_whose_site_is_no_longer_saved_asks_for_a_sign_in() {
    assert!(matches!(
        jira::write::perform(None, &assign("PROJ-1")),
        Err(JiraError::SignIn { site }) if site == "acme.atlassian.net"
    ));
}
