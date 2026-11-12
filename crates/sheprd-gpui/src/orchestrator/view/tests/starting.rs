//! A dispatch the worker took shows "Starting agent…" and blocks a second
//! dispatch until a run or an error arrives. The worker takes no actions in
//! these tests, so the mark is set through the view.
use super::*;
use crate::orchestrator::{Error, Notice, RunState, view::rows::Tab};
use gpui::{Modifiers, TestAppContext};
use std::sync::Arc;

type Page<'a> = (
    tempfile::TempDir,
    gpui::Entity<crate::orchestrator::view::OrchestratorView>,
    &'a mut gpui::VisualTestContext,
);

fn open_page<'a>(cx: &'a mut TestAppContext, issues: Vec<Item>, open: &Item) -> Page<'a> {
    let (dir, view, cx) = empty::open(cx);
    let key = open.key.canonical();
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            let repo = view.repo_mut();
            repo.snapshot.items = Arc::new(issues);
            repo.snapshot.access = Some(crate::orchestrator::Access::ReadWrite);
            view.tab = Tab::Issues;
            view.refresh_rows();
            view.open_detail(key, cx);
        })
    });
    cx.run_until_parked();
    (dir, view, cx)
}

fn issue(id: &str) -> Item {
    item(Provider::Github, id, "Icons", "2026-10-04T00:00:00Z")
}

fn click_dispatch(cx: &mut gpui::VisualTestContext) {
    let bounds = cx
        .debug_bounds("orchestrator-detail-dispatch")
        .unwrap_or_else(|| panic!("the Dispatch button"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

fn mark(
    cx: &mut gpui::VisualTestContext,
    view: &gpui::Entity<crate::orchestrator::view::OrchestratorView>,
    key: String,
) {
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.mark_starting(key);
            cx.notify();
        })
    });
    cx.run_until_parked();
}

fn dialog_open(cx: &mut gpui::VisualTestContext) -> bool {
    cx.debug_bounds("orchestrator-overlay-card").is_some()
}

#[gpui::test]
fn an_unmarked_item_opens_the_dialog(cx: &mut TestAppContext) {
    let one = issue("1");
    let (_dir, _view, cx) = open_page(cx, vec![one.clone()], &one);
    click_dispatch(cx);
    assert!(dialog_open(cx));
}

#[gpui::test]
fn a_starting_item_says_so_and_ignores_the_button(cx: &mut TestAppContext) {
    let one = issue("1");
    let (_dir, view, cx) = open_page(cx, vec![one.clone()], &one);
    mark(cx, &view, one.key.canonical());
    click_dispatch(cx);
    assert!(!dialog_open(cx));
    cx.update(|_, cx| assert!(view.read(cx).is_starting(&one.key.canonical())));
}

#[gpui::test]
fn the_mark_goes_when_a_run_for_the_item_appears(cx: &mut TestAppContext) {
    let one = issue("1");
    let other = issue("2");
    let (_dir, view, cx) = open_page(cx, vec![one.clone(), other.clone()], &one);
    mark(cx, &view, one.key.canonical());
    cx.update(|_, cx| {
        view.update(cx, |view, _| {
            // A run for another item answers nothing.
            let elsewhere = run("r2", &other, RunState::Running, "2026-10-04T01:00:00Z");
            view.repo_mut().snapshot.runs = Arc::new(vec![elsewhere]);
            view.settle_starting(None, true);
            assert!(view.is_starting(&one.key.canonical()));
            let mine = run("r1", &one, RunState::Running, "2026-10-04T01:00:00Z");
            view.repo_mut().snapshot.runs = Arc::new(vec![mine]);
            view.settle_starting(None, true);
            assert!(!view.is_starting(&one.key.canonical()));
        })
    });
}

#[gpui::test]
fn an_error_notice_gives_the_button_back(cx: &mut TestAppContext) {
    let one = issue("1");
    let (_dir, view, cx) = open_page(cx, vec![one.clone()], &one);
    mark(cx, &view, one.key.canonical());
    // A reading with no news changes nothing.
    cx.update(|_, cx| view.update(cx, |view, _| view.settle_starting(None, true)));
    assert!(cx.update(|_, cx| view.read(cx).is_starting(&one.key.canonical())));
    let failed = Notice {
        outcome: Err(Arc::new(Error::Busy)),
    };
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.settle_starting(Some(&failed), false);
            cx.notify();
        })
    });
    cx.run_until_parked();
    click_dispatch(cx);
    assert!(dialog_open(cx));
}

#[gpui::test]
fn marking_one_item_leaves_another_page_alone(cx: &mut TestAppContext) {
    let one = issue("1");
    let two = issue("2");
    let (_dir, view, cx) = open_page(cx, vec![one.clone(), two.clone()], &two);
    mark(cx, &view, one.key.canonical());
    click_dispatch(cx);
    assert!(dialog_open(cx));
}
