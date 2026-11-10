//! Mine on a Gitea repository matches the pull requests the worker found
//! the reader opened, by the sign-in name it read.
use super::*;
use crate::orchestrator::{
    PullRequest, Request, Timing,
    repo::{Remote, RepoInfo},
    view::{Look, OrchestratorView, rows::Tab},
};
use gpui::{TestAppContext, VisualTestContext};
use std::{collections::HashSet, sync::Arc};

fn gitea() -> SourceKey {
    SourceKey {
        provider: Provider::Gitea,
        host: "gitea.arpa".into(),
        repository: "ops/infra".into(),
    }
}

fn pull(id: u64, author: &str) -> Item {
    let mut pull = item(
        Provider::Gitea,
        &format!("pr/{id}"),
        "Change",
        "2026-10-04T00:00:00Z",
    );
    pull.key.source = gitea();
    pull.identifier = format!("#{id}");
    pull.state = "open".into();
    pull.author = Some(author.into());
    pull.pull_request = Some(PullRequest {
        number: id,
        additions: None,
        deletions: None,
        base_ref: "main".into(),
        head_ref: format!("feature/{id}"),
        base_sha: String::new(),
        head_sha: String::new(),
        head_repository: None,
    });
    pull
}

fn listed(view: &gpui::Entity<OrchestratorView>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        let view = view.read(cx);
        view.keys()
            .into_iter()
            .map(|key| view.item(&key).unwrap().identifier.clone())
            .collect()
    })
}

#[gpui::test]
fn mine_on_gitea_keeps_the_pull_requests_the_reader_opened(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let request = Request {
        target: sheprd_client::ConnectTarget::Local,
        checkout: dir.path().join("infra").to_string_lossy().into_owned(),
        workspace_id: "w1".into(),
        token: None,
        data_root: Some(dir.path().join("data")),
        azure_store: Default::default(),
        gitea_store: Default::default(),
        jira_store: Default::default(),
        gitlab_store: Default::default(),
        timing: Timing::default(),
    };
    let config = crate::config::Config::default();
    let look = Look {
        theme: crate::config::Theme::default(),
        ui: config.ui.clone(),
        mono: config.terminal.clone(),
    };
    let (view, cx) = cx.add_window_view(|_, cx| OrchestratorView::with_repo(request, look, cx));
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            let checkout = view.repo_mut();
            checkout.snapshot.repo = Some(RepoInfo {
                repository: crate::orchestrator::Repository::Local(dir.path().join(".git")),
                main_root: dir.path().to_string_lossy().into_owned(),
                beads: false,
                beads_root: None,
                remote: Some(Remote {
                    name: "origin".into(),
                    url: "http://gitea.arpa/ops/infra.git".into(),
                    source: gitea(),
                }),
            });
            // The first was opened by the reader; the second only reads like it.
            let own = pull(1, "alice");
            let other = pull(2, "bob");
            checkout.snapshot.authored = Arc::new(HashSet::from([own.key.canonical()]));
            checkout.snapshot.items = Arc::new(vec![own, other]);
            checkout.snapshot.viewer = Some("alice".into());
            view.tab = Tab::PullRequests;
            view.refresh_rows();
        })
    });
    assert_eq!(listed(&view, cx), ["#1", "#2"]);
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            assert_eq!(view.repos[0].reading_login(), Some("alice"));
            view.yours.authored = true;
            view.refresh_rows();
        })
    });
    assert_eq!(listed(&view, cx), ["#1"]);
}
