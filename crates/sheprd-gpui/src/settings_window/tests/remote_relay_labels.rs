//! Settings > Remote control > Relays with labelled relays: each row says
//! what `[remote] relays` calls it, and editing the list keeps the labels.
use super::remote_control::{click, draw, page};
use super::*;
use crate::{
    config::{
        preferences::Preference,
        remote::{Edit, Relay},
    },
    settings_window::pages::remote_control::{Standing, relay_line},
};
use sheprd_remote_control::relay_label::RelayLabel;

fn fra3() -> Relay {
    Relay {
        endpoint: "https://fra3.example.org".into(),
        label: RelayLabel {
            description: Some("not for public use".into()),
            name: Some("fra3".into()),
            tags: vec!["eu".into(), "private".into()],
        },
    }
}

fn plain(line: String) -> String {
    line.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_labelled_row_names_its_address_description_and_tags() {
    assert_eq!(
        plain(relay_line(&fra3(), &Standing::Listening(None))),
        "Listening · https://fra3.example.org · not for public use · Tags: eu, private"
    );
    // A relay without a label is titled by its address, said once.
    assert_eq!(
        plain(relay_line(
            &"https://sfo3.example.org".into(),
            &Standing::Listening(None)
        )),
        "Listening"
    );
    // A tag alone: no address line, since the address is the title.
    let tagged = Relay {
        label: RelayLabel {
            tags: vec!["eu".into()],
            ..RelayLabel::default()
        },
        ..fra3()
    };
    assert_eq!(
        plain(relay_line(&tagged, &Standing::Connecting)),
        "Connecting\u{2026} · Tags: eu"
    );
}

#[gpui::test]
fn removing_a_relay_keeps_the_others_labels(cx: &mut TestAppContext) {
    let (view, cx, edits) = page(cx);
    view.update(cx, |view, cx| {
        view.config.remote.relays = vec!["https://sfo3.example.org".into(), fra3()];
        cx.notify();
    });
    draw(cx);
    click(cx, "remote-relay-0-remove");
    assert_eq!(
        *edits.lock().unwrap(),
        [Preference::Remote(Edit::Relays(vec![fra3()]))]
    );
}
