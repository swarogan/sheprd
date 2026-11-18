//! Dropped files on their way to a remote chat's host: a chip for each
//! that fills as it is sent, a line with the batch's bytes in the footer,
//! and a cancel that stops the copy.

use super::{
    overlay::{attach_with, open, poll},
    *,
};
use crate::upload_chips::Upload;
use gpui::{Modifiers, TestAppContext};
use sheprd_client::ConnectTarget;

fn idle() -> ServerMessage {
    let mut transcript = Transcript::new("Claude");
    transcript.status = Status::Idle;
    transcript.entries = vec![agent(1, "Hi.")];
    ServerMessage::Snapshot {
        cursor: cursor(1, 0),
        transcript: Box::new(transcript),
    }
}

#[gpui::test]
fn each_file_copying_to_the_host_shows_its_progress_and_can_be_cancelled(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let _stream = attach_with(&view, &program, idle(), cx);
    poll(&view, &program, cx);
    // Seeded as the worker leaves it, on a chat whose agent runs on a host;
    // polling again would rebind the pane to the fixture's local route.
    view.update(cx, |view, cx| {
        let pane = view.chats.panes_mut().next().unwrap();
        pane.target.route = Route::Ssh("devbox".into());
        // The first file is out, the second halfway, the third waits.
        pane.uploads.push(Upload::sample(
            &["crash.log", "shot.png", "trace.json"],
            vec![2_000_000, 8_000_000, 2_000_000],
            6_000_000,
        ));
        pane.uploading = 3;
        view.redraw_terminal(cx);
        cx.notify();
    });
    cx.run_until_parked();
    for chip in ["chat-upload-0", "chat-upload-1", "chat-upload-2"] {
        assert!(cx.debug_bounds(chip).is_some(), "{chip}");
    }
    assert!(cx.debug_bounds("chat-upload-3").is_none());
    // The batch's own chips stand in for the old "Copying 3 files" one.
    assert!(cx.debug_bounds("chat-uploading").is_none());
    assert!(cx.debug_bounds("chat-upload-status").is_some());

    let cancel = cx.debug_bounds("chat-upload-cancel-2").unwrap();
    cx.simulate_click(cancel.center(), Modifiers::default());
    view.read_with(cx, |view, _| {
        let pane = view.chats.panes().next().unwrap();
        assert!(pane.uploads[0].cancelled());
    });
}
