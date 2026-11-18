use super::*;

#[gpui::test]
fn a_reused_checkout_finishing_on_another_host_keeps_focus_and_reports_its_result(
    cx: &mut TestAppContext,
) {
    let (view, cx, id) = window(cx, LINKED, Some(true));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let origin = view.endpoints[0].id.clone();
            let scope = crate::browser::scope(&view.endpoints[0]);
            let target = view.endpoints[0].connection.target.clone();
            view.endpoints[0].label = "Original host".into();
            view.endpoints.push(crate::endpoint::Endpoint::new(
                "ssh:other".into(),
                "Other host".into(),
                sheprd_client::ConnectTarget::Ssh {
                    target: "other.invalid".into(),
                    session: "work".into(),
                },
                false,
            ));
            // Host selection retires the old dialog; a new dialog belongs to
            // the newly selected host, even while the local Git worker runs.
            view.menu.reset();
            view.selected_endpoint = 1;
            view.selection_epoch += 1;
            let epoch = view.selection_epoch;
            let page = Some(Page::Dialog(WorkspaceAction::Rename));
            view.menu.page = page;
            view.menu.creation = Some("another-request".into());

            for pulled in [Ok(()), Err(crate::Error::NotConnected)] {
                let success = pulled.is_ok();
                view.kept_worktree_done.push(KeptDone::Checkout {
                    request: "kept-reuse".into(),
                    landing: Box::new(KeptLanding::reused(&origin, target.clone(), pulled)),
                    result: Ok((Ask::Open { path: NEW.into() }, NEW.into())),
                });
                view.poll_kept_worktree(window, cx);

                assert_eq!(view.selected_endpoint, 1);
                assert_eq!(view.selection_epoch, epoch);
                assert_eq!(view.menu.page, page);
                assert_eq!(view.menu.creation.as_deref(), Some("another-request"));
                assert!(view.pending_navigation.is_none());
                assert!(view.kept_workspace_in(&scope, Path::new(NEW), cx).is_none());
                assert!(view.sessions(0).workspace(&id).is_some());
                assert!(view.kept_worktree_done.is_empty());
                let notice = &view.endpoints[1].toasts.entries.back().unwrap().1;
                assert_eq!(notice.title, "Worktree already exists");
                let body = notice.body.as_deref().unwrap();
                assert!(body.starts_with("Original host: "), "{body}");
                assert!(
                    body.contains(if success {
                        "Pulled the existing worktree's remote."
                    } else {
                        "Could not pull the existing worktree's remote:"
                    }),
                    "{body}"
                );
            }
        });
    });
}

#[gpui::test]
fn a_reused_checkout_still_opens_when_its_host_is_selected_after_dismissing_the_dialog(
    cx: &mut TestAppContext,
) {
    let (view, cx, _) = window(cx, LINKED, Some(true));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let origin = view.endpoints[0].id.clone();
            view.menu.reset();
            view.kept_worktree_done.push(KeptDone::Checkout {
                request: "kept-reuse".into(),
                landing: Box::new(KeptLanding::reused(
                    &origin,
                    view.endpoints[0].connection.target.clone(),
                    Ok(()),
                )),
                result: Ok((Ask::Open { path: NEW.into() }, NEW.into())),
            });
            view.poll_kept_worktree(window, cx);

            assert_eq!(view.selected_endpoint, 0);
            let kept = view.kept_workspace_at(Path::new(NEW)).unwrap();
            assert_eq!(view.kept_in_front(), Some(kept.as_str()));
            assert_eq!(
                view.endpoints[0]
                    .toasts
                    .entries
                    .back()
                    .unwrap()
                    .1
                    .body
                    .as_deref(),
                Some("Pulled the existing worktree's remote.")
            );
        });
    });
}
