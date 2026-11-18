use super::*;
use crate::teleport::job::Landed;
use core::prelude::v1::test;

#[gpui::test]
fn a_closed_arrival_tab_is_not_reopened_by_recovery(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let scope = cx.update(|_, cx| view.read(cx).endpoint_scope("local"));
    let terminal = TerminalId::new("t-closed-arrival").unwrap();
    let outcome = Outcome {
        source_retired: false,
        endpoint_id: "local".into(),
        workspace: Landed::New {
            checkout: "/recovered/closed".into(),
            label: None,
        },
        repo_key: "/recovered/.git".into(),
        branch: "closed".into(),
        warnings: Vec::new(),
        core_tabs: vec![Panes::single(terminal.clone())],
    };
    let path = PathBuf::from("/nonexistent/teleport-arrivals/closed-arrival.json");
    let receipt = || Receipt::new(scope.as_str(), &outcome);
    let mut entries = HashMap::new();
    let held = |cx: &App| -> Vec<crate::browser::TabId> {
        cx.global::<Store>()
            .in_scope(&scope)
            .filter(|tab| matches!(&tab.location, Some(Location::CoreTerminal { panes, .. }) if panes.contains(&terminal)))
            .map(|tab| tab.id)
            .collect()
    };
    let recover = |entries: &mut HashMap<PathBuf, Adopted>, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            if let Some((scope, outcome)) = admit(entries, path.clone(), receipt()) {
                view.update(cx, |view, cx| {
                    view.settle_teleport(&outcome, &scope, window, cx)
                });
            }
            for (scope, outcome) in track_entries(entries, cx.global::<Store>()) {
                view.update(cx, |view, cx| {
                    view.settle_teleport(&outcome, &scope, window, cx)
                });
            }
        });
    };

    recover(&mut entries, cx);
    let tabs = cx.update(|_, cx| held(cx));
    assert_eq!(tabs.len(), 1, "recovery opens the arrival once");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.close_browser_tab(tabs[0], window, cx));
    });

    // The receipt is still on disk until the saved tabs catch up.
    recover(&mut entries, cx);
    assert!(
        cx.update(|_, cx| held(cx)).is_empty(),
        "a closed tab stays closed"
    );
    assert_eq!(
        entries.get(&path).map(|adopted| adopted.closed.clone()),
        Some(vec![terminal.clone()]),
        "its receipt no longer waits for the closed tab"
    );
}

#[gpui::test]
fn arrival_tabs_a_limit_left_out_open_once_room_is_freed(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(crate::sidebar::layout_tests::fixture_window);
    let scope = cx.update(|_, cx| view.read(cx).endpoint_scope("local"));
    let arrival = |name: &str| {
        let terminal = TerminalId::new(name).unwrap();
        let outcome = Outcome {
            source_retired: false,
            endpoint_id: "local".into(),
            workspace: Landed::New {
                checkout: format!("/recovered/{name}"),
                label: None,
            },
            repo_key: "/recovered/.git".into(),
            branch: name.into(),
            warnings: Vec::new(),
            core_tabs: vec![Panes::single(terminal.clone())],
        };
        (terminal, Receipt::new(scope.as_str(), &outcome))
    };
    let held = |terminal: &TerminalId, cx: &App| {
        cx.global::<Store>()
            .in_scope(&scope)
            .any(|tab| matches!(&tab.location, Some(Location::CoreTerminal { panes, .. }) if panes.contains(terminal)))
    };
    let (freed, freed_receipt) = arrival("t-limit-freed");
    let (full, full_receipt) = arrival("t-limit-full");
    let freed_path = PathBuf::from("/nonexistent/teleport-arrivals/limit-freed.json");
    let full_path = PathBuf::from("/nonexistent/teleport-arrivals/limit-full.json");
    cx.update(|window, cx| {
        let room = Store::update(cx, |store| Room::of(store));
        // Both were adopted while a limit kept their tabs out. The app held
        // one more tab then for the first, as many as now for the second.
        let mut entries = HashMap::from([
            (freed_path, Adopted {
                room: Room { tabs: room.tabs + 1, ..room },
                ..Adopted::new(scope.clone(), freed_receipt)
            }),
            (full_path, Adopted {
                room,
                ..Adopted::new(scope.clone(), full_receipt)
            }),
        ]);
        for (scope, outcome) in track_entries(&mut entries, cx.global::<Store>()) {
            view.update(cx, |view, cx| {
                view.settle_teleport(&outcome, &scope, window, cx)
            });
        }
        assert!(held(&freed, cx), "freed room retries the missing tab");
        assert!(!held(&full, cx), "no retry until the user frees room");
        assert!(track_entries(&mut entries, cx.global::<Store>()).is_empty());

        let tab = cx.global::<Store>().in_scope(&scope).find(|tab|
            matches!(&tab.location, Some(Location::CoreTerminal { panes, .. }) if panes.contains(&freed)))
            .unwrap().id;
        view.update(cx, |view, cx| view.close_browser_tab(tab, window, cx));
        for (scope, outcome) in track_entries(&mut entries, cx.global::<Store>()) {
            view.update(cx, |view, cx| {
                view.settle_teleport(&outcome, &scope, window, cx)
            });
        }
        assert!(held(&full, cx), "closing a tab frees room for the other arrival");
        assert!(!held(&freed, cx), "the closed arrival stays closed");
    });
}
