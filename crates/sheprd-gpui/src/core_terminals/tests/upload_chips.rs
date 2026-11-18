//! Files dropped on a terminal on another host show as chips along the
//! pane's foot while they copy, go once their paths are pasted, and a
//! chip's × cancels the drop.

use super::links::shown;
use crate::core_terminals::files::Prepared;
use crate::upload_chips::Upload;
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use sheprd_client::ConnectTarget;

fn copying(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<crate::core_terminals::view::CoreTerminalView>,
    &mut VisualTestContext,
    std::sync::mpsc::SyncSender<crate::Result<Prepared>>,
) {
    let (view, cx, _, _) = shown(cx, &["$"], false);
    let answer = view.update(cx, |view, cx| {
        view.set_paste_target(Some(ConnectTarget::Ssh {
            target: "devbox".into(),
            session: String::new(),
        }));
        // The first file is out, the second halfway, the third waits.
        let answer = view.pending_upload_for_test(Upload::sample(
            &["crash.log", "shot.png", "trace.json"],
            vec![2_000_000, 8_000_000, 2_000_000],
            6_000_000,
        ));
        cx.notify();
        answer
    });
    cx.run_until_parked();
    (view, cx, answer)
}

#[gpui::test]
fn files_copying_to_the_host_show_until_their_paths_are_pasted(cx: &mut TestAppContext) {
    let (view, cx, answer) = copying(cx);
    for chip in [
        "terminal-upload-0",
        "terminal-upload-1",
        "terminal-upload-2",
    ] {
        assert!(cx.debug_bounds(chip).is_some(), "{chip}");
    }
    assert!(cx.debug_bounds("terminal-upload-status").is_some());

    answer.send(Err(crate::Error::ConnectionNotReady)).unwrap();
    cx.update(|window, cx| view.update(cx, |view, cx| view.poll(window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-uploads").is_none());
}

#[gpui::test]
fn a_chips_cancel_stops_the_copy(cx: &mut TestAppContext) {
    let (view, cx, _answer) = copying(cx);
    let cancel = cx.debug_bounds("terminal-upload-cancel-1").unwrap();
    cx.simulate_click(cancel.center(), Modifiers::default());
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.poll(window, cx);
            assert!(matches!(
                view.take_paste_error(),
                Some(crate::Error::Client(sheprd_client::Error::UploadCancelled))
            ));
        })
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-uploads").is_none());
}

#[gpui::test]
fn chips_redraw_once_the_sizes_are_known(cx: &mut TestAppContext) {
    let (view, cx, _, _) = shown(cx, &["$"], false);
    let mut file = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut file, &[0; 4096]).unwrap();
    let paths = [file.path().to_owned()];
    let upload = Upload::new(&paths);
    let reporter = upload.reporter();
    let _answer = view.update(cx, |view, cx| {
        view.set_paste_target(Some(ConnectTarget::Ssh {
            target: "devbox".into(),
            session: String::new(),
        }));
        let answer = view.pending_upload_for_test(upload);
        cx.notify();
        answer
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-upload-0").is_some());
    assert!(cx.debug_bounds("terminal-upload-status").is_none());

    // No byte has gone out yet, as while SSH connects: the sizes alone
    // are news to draw.
    reporter.measure(&paths);
    cx.update(|window, cx| view.update(cx, |view, cx| view.poll(window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("terminal-upload-status").is_some());
}
