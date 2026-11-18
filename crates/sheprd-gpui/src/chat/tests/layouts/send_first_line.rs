use super::*;
use sheprd_chat_protocol::{Choice, ConfigOption, OptionCategory, OptionKind, Usage};

/// When the settings need more than one line, as with a larger font or a
/// narrow pane, they wrap on their own while context use and send stay at
/// the end of the first line.
#[gpui::test]
fn send_stays_on_the_first_line_when_settings_wrap(cx: &mut TestAppContext) {
    let (view, _peer, cx) = open(cx, ConnectTarget::Local);
    let program = Program::new();
    let mut transcript = Transcript::new("Claude");
    transcript.status = Status::Working;
    transcript.entries = vec![agent(1, "Looking.")];
    transcript.usage = Some(Usage {
        used: 253_500,
        size: 1_000_000,
        cost: None,
    });
    transcript.options = [
        (
            "model",
            OptionCategory::Model,
            "Opus 5.5 (recommended) with a long model name",
        ),
        ("effort", OptionCategory::ThoughtLevel, "Auto"),
        ("mode", OptionCategory::Mode, "Bypass Permissions"),
    ]
    .into_iter()
    .map(|(id, category, name)| ConfigOption {
        id: id.into(),
        category,
        name: id.into(),
        description: None,
        kind: OptionKind::Select {
            choices: vec![Choice {
                id: "selected".into(),
                name: name.into(),
                description: None,
                group: None,
            }],
            current: "selected".into(),
        },
    })
    .collect();
    let _stream = attach_with(
        &view,
        &program,
        ServerMessage::Snapshot {
            cursor: cursor(1, 0),
            transcript: Box::new(transcript),
        },
        cx,
    );
    for mode in [
        ChatLayoutMode::Timeline,
        ChatLayoutMode::Document,
        ChatLayoutMode::Cards,
        ChatLayoutMode::IA,
    ] {
        switch(&view, mode, cx);
        view.update(cx, |view, cx| {
            view.config.chat.max_width = 420;
            cx.notify();
        });
        poll(&view, &program, cx);
        let attach = cx.debug_bounds("chat-attach").unwrap();
        let mode_setting = cx.debug_bounds("chat-setting-mode").unwrap();
        assert!(
            mode_setting.top() > attach.bottom(),
            "{mode:?}: the settings wrap"
        );
        let stop = cx.debug_bounds("chat-stop").unwrap();
        let usage = cx.debug_bounds("chat-usage").unwrap();
        assert!(
            stop.top() < attach.bottom(),
            "{mode:?}: stop on the first line"
        );
        assert!(usage.top() < attach.bottom(), "{mode:?}: usage beside stop");
        assert!(usage.right() <= stop.left(), "{mode:?}");
        assert!(
            mode_setting.right() <= usage.left(),
            "{mode:?}: settings keep clear of usage"
        );
    }
}
