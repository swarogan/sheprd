use super::*;
use crate::relay_label::{self, RelayLabel};

#[test]
fn each_label_option_names_the_relay_before_it() {
    let options = serve(&[
        "serve",
        "--state-dir",
        "/s",
        "--relay",
        "https://sfo3.example.org",
        "--relay",
        "https://fra3.example.org",
        "--relay-name",
        "fra3",
        "--relay-description",
        "not for public use",
        "--relay-tag",
        "eu",
        "--relay-tag",
        "private",
    ]);
    let labels: Vec<_> = options
        .relays
        .iter()
        .map(|relay| (relay.endpoint.to_string(), relay.label.clone()))
        .collect();
    assert_eq!(
        labels,
        [
            ("https://sfo3.example.org".to_owned(), RelayLabel::default()),
            (
                "https://fra3.example.org".to_owned(),
                RelayLabel {
                    description: Some("not for public use".into()),
                    name: Some("fra3".into()),
                    tags: vec!["eu".into(), "private".into()],
                }
            ),
        ]
    );
}

#[test]
fn a_label_needs_a_relay_and_must_be_usable() {
    assert_eq!(
        parse(args(&[
            "serve",
            "--state-dir",
            "/s",
            "--relay-name",
            "fra3"
        ])),
        Err(ParseError::NoRelay {
            option: "--relay-name"
        })
    );
    assert_eq!(
        parse(args(&[
            "serve",
            "--state-dir",
            "/s",
            "--relay",
            "https://fra3.example.org",
            "--relay-tag",
            " ",
        ])),
        Err(ParseError::RelayLabel {
            relay: "https://fra3.example.org".into(),
            source: relay_label::Error::Blank { field: "tag" },
        })
    );
}
