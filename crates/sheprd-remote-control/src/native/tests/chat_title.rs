//! A chat no one named shows its first prompt rather than the tab label
//! Herdr fills in; a name the pane or its agent set still wins.

use super::*;
use crate::control::pull_requests::PullRequests;
use model::{Home, Sources};

fn title(named: bool, first_prompt: Option<&str>) -> String {
    let mut layout = layout(EndpointId::random());
    layout.workspaces[0].chats[0].title = "Tab 2".into();
    layout.workspaces[0].chats[0].named = named;
    let summary = Summary {
        first_prompt: first_prompt.map(str::to_owned),
        ..Summary::default()
    };
    let home = Home::project(Sources {
        checkouts: &Default::default(),
        recognized: &Default::default(),
        computer: "Studio",
        fingerprint: "FP",
        layout: &layout,
        load: None,
        summaries: [("p1".to_owned(), summary)].into(),
        pull_requests: &PullRequests::default(),
        shortcuts_ms: 0,
        tags: Default::default(),
        version: "20261115.1",
        waiting: 0,
    });
    home.workspaces[0].chats[0].title.clone()
}

#[test]
fn the_first_prompt_replaces_a_stand_in_title() {
    assert_eq!(title(false, Some("Fix the login")), "Fix the login");
    assert_eq!(title(false, None), "Tab 2");
}

#[test]
fn a_set_title_beats_the_first_prompt() {
    assert_eq!(title(true, Some("Fix the login")), "Tab 2");
}
