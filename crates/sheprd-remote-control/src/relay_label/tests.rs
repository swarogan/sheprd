#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use serde_json::json;

fn fra3() -> RelayLabel {
    RelayLabel {
        description: Some("not for public use".into()),
        name: Some("fra3".into()),
        tags: vec!["eu".into(), "private".into()],
    }
}

#[test]
fn a_label_checks_each_field() {
    assert_eq!(fra3().check(), Ok(()));
    assert_eq!(RelayLabel::default().check(), Ok(()));
    let blank = RelayLabel {
        name: Some("  ".into()),
        ..RelayLabel::default()
    };
    assert_eq!(blank.check(), Err(Error::Blank { field: "name" }));
    let broken = RelayLabel {
        description: Some("two\nlines".into()),
        ..RelayLabel::default()
    };
    assert_eq!(
        broken.check(),
        Err(Error::Control {
            field: "description"
        })
    );
    let long = RelayLabel {
        tags: vec!["t".repeat(TAG_MAX + 1)],
        ..RelayLabel::default()
    };
    assert_eq!(
        long.check(),
        Err(Error::Length {
            field: "tag",
            max: TAG_MAX
        })
    );
    // Characters, not bytes, count toward the limit.
    let wide = RelayLabel {
        name: Some("é".repeat(NAME_MAX)),
        ..RelayLabel::default()
    };
    assert_eq!(wide.check(), Ok(()));
    let many = RelayLabel {
        tags: (0..=TAGS_MAX).map(|tag| tag.to_string()).collect(),
        ..RelayLabel::default()
    };
    assert_eq!(many.check(), Err(Error::Tags));
}

#[test]
fn a_label_leaves_out_what_it_does_not_say() {
    assert_eq!(
        serde_json::to_value(RelayLabel::default()).unwrap(),
        json!({})
    );
    assert_eq!(
        serde_json::to_value(fra3()).unwrap(),
        json!({"description": "not for public use", "name": "fra3", "tags": ["eu", "private"]})
    );
    assert!(RelayLabel::default().is_empty());
    assert!(!fra3().is_empty());
}

#[test]
fn a_label_becomes_the_daemons_options() {
    assert_eq!(
        fra3().args().collect::<Vec<_>>(),
        [
            ("--relay-name", "fra3"),
            ("--relay-description", "not for public use"),
            ("--relay-tag", "eu"),
            ("--relay-tag", "private"),
        ]
    );
    assert_eq!(RelayLabel::default().args().count(), 0);
}
