use super::*;
use crate::gitea::{self, Repository, saved::Saved};
use crate::repo::parse_remote_url;
use serde_json::{Value, json};

const BASE: &str = "http://gitea.arpa";

fn source() -> SourceKey {
    SourceKey {
        provider: Provider::Gitea,
        host: "gitea.arpa".into(),
        repository: "ops/infra".into(),
    }
}

fn user(login: &str) -> Value {
    json!({ "id": 3, "login": login, "full_name": "", "email": "x@example.test" })
}

fn issue(number: u64) -> Value {
    json!({
        "id": number + 1000, "number": number, "title": format!("Issue {number}"),
        "body": "Steps\n\n- one\n- **two**", "state": "open", "comments": 2,
        "user": user("alice"),
        "labels": [{ "id": 1, "name": "bug", "color": "ee0701" }],
        "assignees": [user("bob")],
        "html_url": "http://localhost:3000/ops/infra/issues/1",
        "created_at": "2026-10-01T10:00:00Z", "updated_at": "2026-10-02T11:30:00+02:00",
        "pull_request": null,
    })
}

fn pull(number: u64, author: &str, extra: Value) -> Value {
    let mut pull = json!({
        "id": number + 2000, "number": number, "title": format!("Change {number}"),
        "body": "", "state": "open", "draft": false, "merged": false,
        "user": user(author), "labels": null, "comments": 1,
        "additions": 10, "deletions": 3,
        "created_at": "2026-10-03T10:00:00Z", "updated_at": "2026-10-04T10:00:00Z",
        "head": { "ref": "feature/x", "sha": "aaa", "repo": { "full_name": "fork/infra" } },
        "base": { "ref": "main", "sha": "bbb", "repo": { "full_name": "ops/infra" } },
    });
    pull.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    pull
}

/// The addresses a fake fetch has been asked.
type Asked = std::rc::Rc<std::cell::RefCell<Vec<String>>>;

/// Answers each request from `answers` in order, recording the addresses asked.
fn replies(answers: Vec<Value>) -> (impl FnMut(url::Url) -> Result<Vec<u8>>, Asked) {
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = asked.clone();
    let mut answers = answers.into_iter();
    let fetch = move |url: url::Url| {
        log.borrow_mut().push(url.to_string());
        let answer = answers.next().unwrap_or_else(|| json!([]));
        Ok(serde_json::to_vec(&answer).unwrap())
    };
    (fetch, asked)
}

#[test]
fn a_listing_keeps_issues_open_pull_requests_and_recently_settled_ones_apart() {
    let (mut fetch, asked) = replies(vec![
        json!([issue(7), issue(3)]),
        json!([
            pull(12, "alice", json!({})),
            pull(13, "bob", json!({ "draft": true, "body": "Why" })),
            pull(14, "alice", json!({})),
        ]),
        json!([
            pull(
                9,
                "alice",
                json!({ "state": "closed", "merged": true, "labels": [{ "name": "ship" }] })
            ),
            pull(8, "bob", json!({ "state": "closed" })),
            // Settled while the open ones were read: listed once, as settled.
            pull(14, "alice", json!({ "state": "closed", "merged": true })),
        ]),
        user("Alice"),
    ]);
    let listing = gitea::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    let ids: Vec<_> = listing
        .items
        .iter()
        .map(|item| (item.key.native_id.as_str(), item.state.as_str()))
        .collect();
    assert_eq!(
        ids,
        [
            ("7", "open"),
            ("3", "open"),
            ("pr/12", "open"),
            ("pr/13", "draft"),
            ("pr/9", "merged"),
            ("pr/8", "closed"),
            ("pr/14", "merged"),
        ]
    );
    assert!(
        listing
            .items
            .iter()
            .all(|item| item.identifier.starts_with('#'))
    );

    let first = &listing.items[0];
    assert_eq!(first.title, "Issue 7");
    assert_eq!(
        first.description.as_deref(),
        Some("Steps\n\n- one\n- **two**")
    );
    assert_eq!(first.labels, ["bug"]);
    assert_eq!(first.author.as_deref(), Some("alice"));
    assert_eq!(
        first.url.as_deref(),
        Some("http://gitea.arpa/ops/infra/issues/7")
    );
    assert_eq!(first.activity.unwrap().comments, Some(2));
    assert_eq!(
        first.updated_at.unwrap().to_rfc3339(),
        "2026-10-02T09:30:00+00:00"
    );
    assert!(first.pull_request.is_none());

    let draft = &listing.items[3];
    assert_eq!(draft.description.as_deref(), Some("Why"));
    assert_eq!(
        draft.url.as_deref(),
        Some("http://gitea.arpa/ops/infra/pulls/13")
    );
    let request = draft.pull_request.as_ref().unwrap();
    assert_eq!(request.number, 13);
    assert_eq!(
        (request.base_ref.as_str(), request.head_ref.as_str()),
        ("main", "feature/x")
    );
    assert_eq!(
        (request.base_sha.as_str(), request.head_sha.as_str()),
        ("bbb", "aaa")
    );
    assert_eq!(request.head_repository.as_deref(), Some("fork/infra"));
    assert_eq!((request.additions, request.deletions), (Some(10), Some(3)));
    // An empty body and no labels are not shown.
    assert_eq!(listing.items[2].description, None);
    assert!(listing.items[2].labels.is_empty());
    assert_eq!(listing.items[4].labels, ["ship"]);

    // The reader is matched by login, whatever the case.
    assert_eq!(listing.viewer.as_deref(), Some("Alice"));
    let mut authored: Vec<_> = listing.authored.iter().cloned().collect();
    authored.sort();
    assert_eq!(
        authored,
        [
            "gitea:gitea.arpa:ops/infra:pr/12",
            "gitea:gitea.arpa:ops/infra:pr/14",
            "gitea:gitea.arpa:ops/infra:pr/9",
        ]
    );

    let asked = asked.borrow();
    assert_eq!(
        asked[0],
        "http://gitea.arpa/api/v1/repos/ops/infra/issues?state=open&type=issues&page=1&limit=50"
    );
    assert_eq!(
        asked[1],
        "http://gitea.arpa/api/v1/repos/ops/infra/pulls?state=open&page=1&limit=50"
    );
    assert_eq!(
        asked[2],
        "http://gitea.arpa/api/v1/repos/ops/infra/pulls?state=closed&sort=recentupdate&page=1&limit=50"
    );
    assert_eq!(asked[3], "http://gitea.arpa/api/v1/user");
}

#[test]
fn a_pull_request_that_slips_into_the_issue_list_is_not_an_issue() {
    let mut stray = issue(5);
    stray["pull_request"] = json!({ "merged": false });
    let (mut fetch, _) = replies(vec![json!([stray, issue(6)])]);
    let listing = gitea::list(&source(), BASE, None, &mut fetch).unwrap();
    let ids: Vec<_> = listing
        .items
        .iter()
        .map(|item| item.key.native_id.as_str())
        .collect();
    assert_eq!(ids, ["6"]);
}

#[test]
fn pages_are_read_until_a_short_one() {
    let full: Vec<Value> = (1..=50).map(issue).collect();
    let (mut fetch, asked) = replies(vec![json!(full), json!([issue(51)])]);
    let listing = gitea::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(listing.complete);
    assert_eq!(listing.items.len(), 51);
    let asked = asked.borrow();
    assert!(asked[0].contains("page=1&limit=50"), "{asked:?}");
    assert!(
        asked[1].contains("/issues?") && asked[1].contains("page=2&limit=50"),
        "{asked:?}"
    );
    // Pull requests start over at their own first page.
    assert!(
        asked[2].contains("/pulls?") && asked[2].contains("page=1"),
        "{asked:?}"
    );
}

#[test]
fn a_listing_past_the_cap_is_incomplete() {
    let page = |start: u64| json!((start..start + 50).map(issue).collect::<Vec<_>>());
    let pages = gitea::MAX_OPEN as u64 / 50;
    let answers = (0..pages).map(|index| page(index * 50 + 1)).collect();
    let (mut fetch, asked) = replies(answers);
    let listing = gitea::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(!listing.complete);
    assert_eq!(listing.items.len(), gitea::MAX_OPEN);
    let issue_requests = asked
        .borrow()
        .iter()
        .filter(|url| url.contains("/issues?"))
        .count();
    assert_eq!(
        issue_requests as u64, pages,
        "no page past the cap is asked for"
    );
}

#[test]
fn a_full_page_that_repeats_the_last_ends_the_listing_as_incomplete() {
    let full: Vec<Value> = (1..=50).map(issue).collect();
    let (mut fetch, asked) = replies(vec![json!(full), json!(full)]);
    let listing = gitea::list(&source(), BASE, None, &mut fetch).unwrap();
    assert!(!listing.complete);
    assert_eq!(
        listing
            .items
            .iter()
            .filter(|item| item.pull_request.is_none())
            .count(),
        50
    );
    let issue_requests = asked
        .borrow()
        .iter()
        .filter(|url| url.contains("/issues?"))
        .count();
    assert_eq!(issue_requests, 2);
}

#[test]
fn a_reply_that_is_not_the_expected_json_is_a_failure() {
    let mut fetch = |_: url::Url| Ok(b"<html>sign in</html>".to_vec());
    let error = gitea::list(&source(), BASE, None, &mut fetch).unwrap_err();
    assert!(
        matches!(error, Error::Gitea(gitea::Error::Json(_))),
        "{error}"
    );
}

#[test]
fn an_unreadable_reader_leaves_the_listing_without_a_viewer() {
    let (mut fetch, _) = replies(vec![
        json!([]),
        json!([]),
        json!([]),
        json!({ "message": "?" }),
    ]);
    let listing = gitea::list(&source(), BASE, None, &mut fetch).unwrap();
    assert_eq!(listing.viewer, None);
    assert!(listing.authored.is_empty());
}

#[test]
fn addresses_follow_the_instances_sub_path_and_port() {
    let key = source();
    let plain = Repository::of(&key, "http://gitea.arpa:3000").unwrap();
    assert_eq!(plain.web().unwrap(), "http://gitea.arpa:3000/ops/infra");
    assert_eq!(
        plain.pull_requests_url(2).unwrap().as_str(),
        "http://gitea.arpa:3000/api/v1/repos/ops/infra/pulls?state=open&page=2&limit=50"
    );

    // A remote on an instance served from a sub-path names that path first.
    let nested = SourceKey {
        repository: "gitea/ops/infra".into(),
        host: "example.com".into(),
        ..key.clone()
    };
    let nested = Repository::of(&nested, "https://example.com/gitea").unwrap();
    assert_eq!((nested.owner, nested.name), ("ops", "infra"));
    assert_eq!(nested.web().unwrap(), "https://example.com/gitea/ops/infra");
    assert_eq!(
        nested.issues_url(1).unwrap().as_str(),
        "https://example.com/gitea/api/v1/repos/ops/infra/issues?state=open&type=issues&page=1&limit=50"
    );
}

#[test]
fn a_repository_without_an_owner_and_name_is_refused() {
    for repository in ["infra", "a/b/c", "/infra", "ops/"] {
        let key = SourceKey {
            repository: repository.into(),
            ..source()
        };
        assert!(
            matches!(Repository::of(&key, BASE), Err(gitea::Error::Address(_))),
            "{repository}"
        );
    }
}

#[test]
fn status_codes_say_what_the_reader_can_do_about_them() {
    for status in [401, 403] {
        let error = gitea::status_error(status, "ops/infra");
        assert!(matches!(error, gitea::Error::Authentication), "{status}");
        assert!(error.needs_sign_in() && error.needs_user());
    }
    let missing = gitea::status_error(404, "ops/infra");
    assert!(matches!(&missing, gitea::Error::NotFound { repository } if repository == "ops/infra"));
    assert!(!missing.needs_sign_in());
    assert!(matches!(
        gitea::status_error(429, "x"),
        gitea::Error::RateLimit
    ));
    assert!(matches!(
        gitea::status_error(500, "x"),
        gitea::Error::Status(500)
    ));
    assert!(!gitea::status_error(500, "x").needs_user());
}

#[test]
fn a_rate_limit_waits_as_long_as_asked_within_bounds() {
    let wait = |status: u16, value: Option<&str>| {
        let mut headers = ureq::http::HeaderMap::new();
        if let Some(value) = value {
            headers.insert("retry-after", value.parse().unwrap());
        }
        gitea::retry_after(status, &headers).map(|wait| wait.as_secs())
    };
    assert_eq!(wait(429, Some("900")), Some(900));
    assert_eq!(wait(429, Some("1")), Some(300));
    assert_eq!(wait(429, Some("999999")), Some(86400));
    assert_eq!(wait(429, None), Some(3600));
    assert_eq!(wait(500, Some("900")), None);
}

#[test]
fn listing_without_a_saved_instance_asks_to_sign_in() {
    let error = gitea::sync(&source(), None, None, &|| false, &mut None).unwrap_err();
    assert!(
        matches!(error, Error::Gitea(gitea::Error::SignIn { .. })),
        "{error}"
    );
}

fn saved_for(address: &str) -> Saved {
    let mut saved = Saved::default();
    saved.set(address, Some("token".into())).unwrap();
    saved
}

#[test]
fn a_remote_on_a_saved_host_becomes_a_gitea_source_keeping_host_and_repository() {
    let saved = saved_for("http://gitea.arpa:3000");
    for (provider, host) in [
        (Provider::Gitlab, "gitea.arpa"),
        (Provider::Github, "gitea.arpa"),
        // A remote's own port does not matter, nor its case.
        (Provider::Github, "gitea.arpa:8080"),
        (Provider::Github, "gitea.arpa"),
    ] {
        let mut key = SourceKey {
            provider,
            host: host.into(),
            repository: "ops/infra".into(),
        };
        assert!(gitea::recognize(&mut key, || &saved), "{host}");
        assert_eq!(
            key,
            SourceKey {
                provider: Provider::Gitea,
                host: host.into(),
                repository: "ops/infra".into()
            }
        );
    }
}

#[test]
fn recognition_leaves_other_hosts_and_github_alone() {
    let mut saved = saved_for("http://gitea.arpa");
    saved
        .set("https://github.com", Some("token".into()))
        .unwrap();
    let key = |provider, host: &str| SourceKey {
        provider,
        host: host.into(),
        repository: "ops/infra".into(),
    };
    for (provider, host) in [
        (Provider::Github, "github.com"),
        (Provider::Github, "github.com:443"),
        (Provider::Github, "other.arpa"),
        (Provider::Azure, "gitea.arpa"),
        (Provider::Gitea, "gitea.arpa"),
    ] {
        let mut source = key(provider, host);
        assert!(!gitea::recognize(&mut source, || &saved), "{host}");
        assert_eq!(source, key(provider, host));
    }
}

#[test]
fn the_saved_record_is_read_only_when_a_guess_could_change() {
    let mut azure = SourceKey {
        provider: Provider::Azure,
        host: "dev.azure.com".into(),
        repository: "a/b/c".into(),
    };
    let mut github = SourceKey {
        provider: Provider::Github,
        host: "github.com".into(),
        repository: "a/b".into(),
    };
    assert!(!gitea::recognize(&mut azure, || -> Saved {
        unreachable!()
    }));
    assert!(!gitea::recognize(&mut github, || -> Saved {
        unreachable!()
    }));
}

#[test]
fn a_remote_url_on_a_saved_host_reads_as_gitea_once_recognized() {
    let saved = saved_for("http://gitea.arpa");
    for url in [
        "http://gitea.arpa/ops/infra.git",
        "git@gitea.arpa:ops/infra.git",
        "ssh://git@gitea.arpa:2222/ops/infra.git",
    ] {
        let mut key = parse_remote_url(url).unwrap();
        assert!(gitea::recognize(&mut key, || &saved), "{url}");
        assert_eq!(key.provider, Provider::Gitea);
        assert_eq!(key.repository, "ops/infra");
    }
}
