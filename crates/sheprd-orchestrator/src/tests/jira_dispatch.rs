//! A Jira issue goes through the shared item plumbing like any other
//! provider's: keys, branch names, prompts, the unlisted stand-in, storage.
use super::*;
use crate::{
    jira,
    naming::{branch, found_runs},
    prompt::{built_in, valid_branch},
};

fn jira_source(host: &str) -> SourceKey {
    jira::source_key(host)
}

fn issue(source: &SourceKey, key: &str, title: &str) -> Item {
    Item {
        identifier: key.into(),
        url: Some(format!("https://{}/browse/{key}", source.host)),
        ..item(source, key, title)
    }
}

#[test]
fn a_jira_key_round_trips_as_a_provider_and_an_item_key() {
    assert_eq!("jira".parse::<Provider>().unwrap(), Provider::Jira);
    assert_eq!(Provider::Jira.as_str(), "jira");
    let key = ItemKey {
        source: jira_source("acme.atlassian.net"),
        native_id: "PROJ-123".into(),
    };
    assert_eq!(key.canonical(), "jira:acme.atlassian.net:assigned:PROJ-123");
    assert_eq!(key.canonical().parse::<ItemKey>().unwrap(), key);
}

#[test]
fn an_unlisted_jira_item_is_called_by_its_key_not_a_number() {
    let key = ItemKey {
        source: jira_source("acme.atlassian.net"),
        native_id: "PROJ-123".into(),
    };
    assert_eq!(Item::stand_in(key).identifier, "PROJ-123");
}

#[test]
fn an_agent_branch_for_a_jira_issue_is_named_by_its_key_and_stays_its_own() {
    let acme = jira_source("acme.atlassian.net");
    let issue = issue(&acme, "PROJ-123", "Fix the login: 500 on retry!");
    let name = branch(&issue);
    assert!(
        name.starts_with("agent/proj-123-fix-the-login-500-on-retry-"),
        "{name}"
    );
    assert!(valid_branch(&name), "{name}");
    // The same key on another site is another issue, so another branch.
    let other = self::issue(
        &jira_source("other.atlassian.net"),
        "PROJ-123",
        "Fix the login: 500 on retry!",
    );
    assert_ne!(branch(&other), name);

    // The branch finds its issue again with no run recorded.
    let found = found_runs(
        std::slice::from_ref(&issue),
        &[],
        &[],
        std::slice::from_ref(&name),
        &[],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].item_key, issue.key.canonical());
}

#[test]
fn the_prompt_names_the_site_since_the_source_is_every_assigned_issue() {
    let acme = jira_source("acme.atlassian.net");
    let mut issue = issue(&acme, "PROJ-123", "Fix login");
    issue.description = Some("Steps".into());
    let prompt = built_in(&issue);
    assert!(
        prompt.starts_with(
            "Implement this issue.\n\nProvider: jira\nRepository: acme.atlassian.net\nIdentifier: PROJ-123\n"
        ),
        "{prompt}"
    );
    assert!(
        prompt.ends_with("URL: https://acme.atlassian.net/browse/PROJ-123"),
        "{prompt}"
    );
    assert!(!prompt.contains("assigned"));
}

// The database insists on a private folder, which is a Unix mode.
#[cfg(unix)]
#[test]
fn a_jira_issue_survives_the_database() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut store = Store::open(&dir.path().join("db.sqlite")).unwrap();
    let source = jira_source("acme.atlassian.net");
    let mut stored = issue(&source, "PROJ-123", "Fix login");
    stored.state = "in_progress".into();
    stored.priority = Some(1);
    stored.parent_id = Some("PROJ-100".into());
    store
        .replace_items(
            &source,
            std::slice::from_ref(&stored),
            &Checkpoint::default(),
        )
        .unwrap();
    assert_eq!(store.items().unwrap(), [stored]);
}
