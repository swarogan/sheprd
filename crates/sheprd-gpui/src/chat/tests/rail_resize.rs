//! Resizing the rail beside an agent chat by its edge.

use super::{
    overlay::{attach_with, open, poll},
    *,
};
use gpui::{Modifiers, MouseButton, MouseDownEvent, TestAppContext, point, px};
use sheprd_chat_protocol::{Activity, ActivityKind, ActivityState};
use sheprd_client::ConnectTarget;

fn shell() -> ServerMessage {
    let mut transcript = Transcript::new("Claude");
    transcript.status = Status::Idle;
    transcript.entries = vec![agent(1, "It runs in the background.")];
    transcript.activities = vec![Activity {
        call_id: "shell".into(),
        detail: None,
        finished_ms: None,
        kind: ActivityKind::Shell,
        label: "npm run dev".into(),
        started_ms: now_ms(),
        state: ActivityState::Running,
        stoppable: false,
    }];
    ServerMessage::Snapshot {
        cursor: cursor(1, 0),
        transcript: Box::new(transcript),
    }
}

#[gpui::test]
fn dragging_the_rail_edge_resizes_it_and_a_double_click_resets(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let _stream = attach_with(&view, &program, shell(), cx);
    let rail = cx.debug_bounds("chat-rail").unwrap();
    assert_eq!(rail.size.width, px(240.));
    let edge = cx.debug_bounds("chat-rail-resize").unwrap();
    assert!(
        (edge.left() - rail.left()).abs() <= px(1.),
        "on the inner edge: {edge:?} {rail:?}"
    );

    // Dragging the edge 60px left widens the rail by as much.
    let start = edge.center();
    let to = point(start.x - px(60.), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(start.x - px(20.), start.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    poll(&view, &program, cx);
    let wider = cx.debug_bounds("chat-rail").unwrap();
    assert!((wider.size.width - px(300.)).abs() <= px(4.), "{wider:?}");
    assert_eq!(wider.right(), rail.right(), "the far edge stays put");
    // Released, the width is handed to the chrome save once; that the file
    // keeps it is `preferences::tests::chat_rail_width`'s concern.
    view.update(cx, |view, _| {
        assert!(view.chat_rail_width.chosen().is_some());
        assert!(
            !view.chat_rail_width.take_unsaved(),
            "handed off on release"
        );
    });

    // Never wider than half the chat, however far it is dragged.
    let edge = cx.debug_bounds("chat-rail-resize").unwrap();
    let far = point(px(0.), edge.center().y);
    cx.simulate_mouse_down(edge.center(), MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        point(edge.center().x - px(40.), edge.center().y),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.simulate_mouse_move(far, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(far, MouseButton::Left, Modifiers::default());
    poll(&view, &program, cx);
    let widest = cx.debug_bounds("chat-rail").unwrap();
    let chat = cx.debug_bounds("chat-view").unwrap();
    assert!(widest.size.width > wider.size.width, "{widest:?}");
    assert!(
        widest.size.width <= chat.size.width * 0.5 + px(1.),
        "{widest:?} of {chat:?}"
    );

    // A double-click on the edge goes back to the default.
    let edge = cx.debug_bounds("chat-rail-resize").unwrap();
    cx.simulate_event(MouseDownEvent {
        position: edge.center(),
        button: MouseButton::Left,
        modifiers: Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    poll(&view, &program, cx);
    assert_eq!(cx.debug_bounds("chat-rail").unwrap().size.width, px(240.));
    view.read_with(cx, |view, _| {
        assert_eq!(view.chat_rail_width.chosen(), None)
    });
}
