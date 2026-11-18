use sheprd_chat_protocol::{Agent, ConfigOption, Harnesses, OptionCategory, OptionKind};

use super::*;

fn thinking() -> ConfigOption {
    ConfigOption {
        category: OptionCategory::ThoughtLevel,
        description: None,
        id: "effort".to_owned(),
        kind: OptionKind::Toggle { current: false },
        name: "Thinking".to_owned(),
    }
}

fn choosing() -> Transcript {
    Transcript {
        agent: "Claude".to_owned(),
        harnesses: Some(Harnesses {
            available: vec![Agent::Claude, Agent::Codex],
            current: Some(Agent::Claude),
        }),
        options: vec![thinking()],
        ..Transcript::default()
    }
}

#[test]
fn the_agent_row_comes_first_while_the_chat_may_switch() {
    let mut view = View::default();
    snapshot(&mut view, choosing());
    assert!(ctrl(&mut view, 'o').is_none());
    assert_eq!(view.settings, Some(0));
    assert_eq!(view.setting_rows(), 2);
    assert_eq!(
        press(&mut view, KeyCode::Right),
        Some(Out::Command(Command::SetAgent {
            agent: Agent::Codex
        }))
    );
    press(&mut view, KeyCode::Down);
    assert_eq!(
        press(&mut view, KeyCode::Right),
        Some(Out::Command(Command::SetOption {
            id: "effort".to_owned(),
            value: sheprd_chat_protocol::OptionValue::Toggle { value: true },
        }))
    );
}

#[test]
fn settings_open_for_the_agent_alone() {
    let mut view = View::default();
    snapshot(
        &mut view,
        Transcript {
            options: Vec::new(),
            ..choosing()
        },
    );
    ctrl(&mut view, 'o');
    assert_eq!(view.settings, Some(0));
}

#[test]
fn the_panel_follows_the_choice_closing() {
    let mut view = View::default();
    let cursor = snapshot(&mut view, choosing());
    ctrl(&mut view, 'o');
    press(&mut view, KeyCode::Down);
    assert_eq!(view.settings, Some(1));
    view.receive(ServerMessage::Change {
        change: Change::Harnesses { harnesses: None },
        cursor: Cursor {
            seq: cursor.seq + 1,
            ..cursor
        },
    });
    // The thinking setting is the only row left.
    assert_eq!(view.settings, Some(0));
    assert_eq!(
        press(&mut view, KeyCode::Right),
        Some(Out::Command(Command::SetOption {
            id: "effort".to_owned(),
            value: sheprd_chat_protocol::OptionValue::Toggle { value: true },
        }))
    );
}
