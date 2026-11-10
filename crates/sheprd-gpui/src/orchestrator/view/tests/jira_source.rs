//! Jira issues are listed in every checkout and belong to a site, not to a
//! project: their row names the site, and a site waiting for a sign-in says so.
use super::*;
use crate::orchestrator::{
    Request, SourceStatus, SyncState, Timing,
    view::{Event, RepoSpec, repo::Repo, tasks::jira_site},
};
use gpui::{Modifiers, TestAppContext};
use std::{cell::RefCell, rc::Rc, sync::Arc};

fn site(host: &str) -> SourceKey {
    SourceKey {
        provider: Provider::Jira,
        host: host.into(),
        repository: "assigned".into(),
    }
}

fn issue(host: &str) -> Item {
    let mut issue = item(Provider::Jira, "PROJ-1", "Fix it", "2026-10-04T00:00:00Z");
    issue.key.source = site(host);
    issue
}

#[test]
fn only_a_jira_issue_names_a_site() {
    assert_eq!(
        jira_site(&issue("acme.atlassian.net")),
        Some("acme.atlassian.net")
    );
    let github = item(Provider::Github, "7", "Scrolling", "2026-10-04T00:00:00Z");
    assert_eq!(jira_site(&github), None);
}

#[gpui::test]
fn a_jira_row_names_its_site_even_when_two_checkouts_list_it(cx: &mut TestAppContext) {
    let (dir, view, cx) = empty::open(cx);
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            let request = Request {
                target: sheprd_client::ConnectTarget::Local,
                checkout: dir.path().join("other").to_string_lossy().into_owned(),
                workspace_id: "other".into(),
                token: None,
                data_root: Some(dir.path().join("other-data")),
                azure_store: Default::default(),
                gitea_store: Default::default(),
                jira_store: Default::default(),
                gitlab_store: Default::default(),
                timing: Timing::default(),
            };
            view.repos.push(Repo::start(RepoSpec {
                key: "other".into(),
                host: "9950x3d.local".into(),
                request,
            }));
            let issue = issue("acme.atlassian.net");
            let key = issue.key.canonical();
            for repo in &mut view.repos {
                repo.snapshot.items = Arc::new(vec![issue.clone()]);
            }
            view.tab = Tab::Issues;
            view.refresh_rows();
            // One row, held by both checkouts, which still says the site
            // rather than "project · 2 hosts".
            assert_eq!(view.keys(), std::slice::from_ref(&key));
            assert_eq!(view.twins(&key), 1);
            let row = view.lines.iter().find_map(|line| match line {
                Line::Row(row) => Some(*row),
                Line::Heading { .. } => None,
            });
            let row = row.unwrap();
            assert_eq!(view.row_project(&row), "acme.atlassian.net");
        })
    });
}

#[gpui::test]
fn a_jira_site_waiting_for_a_sign_in_offers_to_connect_jira(cx: &mut TestAppContext) {
    let (_dir, view, cx) = empty::open(cx);
    let asked = Rc::new(RefCell::new(Vec::new()));
    let seen = asked.clone();
    cx.update(|_, cx| {
        cx.subscribe(&view, move |_, event: &Event, _| {
            if let Event::SignIn(provider) = event {
                seen.borrow_mut().push(*provider);
            }
        })
        .detach();
        view.update(cx, |view, cx| {
            view.repo_mut().snapshot.sources = vec![SourceStatus {
                key: site("acme.atlassian.net"),
                state: SyncState::SignIn,
            }];
            cx.notify();
        });
    });
    cx.run_until_parked();
    let banner = cx.debug_bounds("orchestrator-banner-sign-in").unwrap();
    cx.simulate_click(banner.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(*asked.borrow(), [Provider::Jira]);
}

#[gpui::test]
fn a_saved_or_forgotten_jira_site_has_every_windows_orchestrator_read_again(
    cx: &mut TestAppContext,
) {
    let notified = Rc::new(RefCell::new(0));
    let mut windows = Vec::new();
    for _ in 0..2 {
        let (window, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
        let seen = notified.clone();
        cx.update(|_, cx| {
            let view = window.update(cx, |window, cx| {
                let id = window.sections_view(cx);
                window.orchestrators[&id].view.clone()
            });
            cx.observe(&view, move |_, _| *seen.borrow_mut() += 1)
                .detach();
        });
        cx.run_until_parked();
        windows.push(window);
    }
    // A refused save changes nothing, so nothing is read again.
    *notified.borrow_mut() = 0;
    cx.update(|cx| {
        windows[0].update(cx, |window, cx| {
            window.run_jira(cx, None, || {
                Err(sheprd_orchestrator::jira::Error::Authentication)
            });
        });
    });
    cx.run_until_parked();
    assert_eq!(*notified.borrow(), 0);
    cx.update(|cx| {
        windows[0].update(cx, |window, cx| {
            window.run_jira(cx, None, || Ok("Forgot.".to_owned()));
        });
    });
    cx.run_until_parked();
    assert_eq!(*notified.borrow(), 2);
}
