use super::*;
use crate::config::remote::Relay;
use sheprd_remote_control::relay_label::{self, RelayLabel};

#[test]
fn a_relay_is_an_address_or_a_labelled_table() -> anyhow::Result<()> {
    let config = Config::parse(
        "[remote]\nrelays = [\n  'https://sfo3.example.org',\n  \
         { endpoint = 'https://fra3.example.org', name = 'fra3', \
         description = 'not for public use', tags = ['eu', 'private'] },\n  \
         { endpoint = 'https://ams3.example.org', name = 'ams3' },\n]\n",
    )?;
    assert!(config.unknown_keys.is_empty(), "{:?}", config.unknown_keys);
    assert_eq!(
        config.remote.relays,
        [
            Relay::from("https://sfo3.example.org"),
            Relay {
                endpoint: "https://fra3.example.org".into(),
                label: RelayLabel {
                    description: Some("not for public use".into()),
                    name: Some("fra3".into()),
                    tags: vec!["eu".into(), "private".into()],
                },
            },
            Relay {
                endpoint: "https://ams3.example.org".into(),
                label: RelayLabel {
                    name: Some("ams3".into()),
                    ..RelayLabel::default()
                },
            },
        ]
    );
    Ok(())
}

#[test]
fn a_relay_label_the_daemon_cannot_use_is_refused() {
    let Err(error) =
        Config::parse("[remote]\nrelays = [{ endpoint = 'https://fra3.example.org', name = '' }]")
    else {
        panic!("a blank name loaded");
    };
    assert!(
        matches!(
            &error,
            crate::Error::RemoteControl(crate::remote::Error::RelayLabel {
                relay,
                source: relay_label::Error::Blank { field: "name" },
            }) if relay == "https://fra3.example.org"
        ),
        "{error:?}"
    );
    for text in [
        // A table needs its address.
        "relays = [{ name = 'fra3' }]",
        // A misspelt key would otherwise drop the label without a word.
        "relays = [{ endpoint = 'https://fra3.example.org', tag = 'eu' }]",
        "relays = [{ endpoint = 'https://fra3.example.org', tags = 'eu' }]",
        "relays = [7]",
    ] {
        assert!(
            Config::parse(&format!("[remote]\n{text}")).is_err(),
            "{text} loaded"
        );
    }
}

#[test]
fn saving_the_list_from_settings_keeps_each_label() -> anyhow::Result<()> {
    use crate::config::{preferences::Preference, remote::Edit};
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("local.toml");
    let fra3 = Relay {
        endpoint: " https://fra3.example.org/ ".into(),
        label: RelayLabel {
            description: Some("not for public use".into()),
            name: Some("fra3".into()),
            tags: vec!["eu".into(), "private".into()],
        },
    };
    Config::save_preference_path(
        Preference::Remote(Edit::Relays(vec![
            "https://sfo3.example.org".into(),
            fra3.clone(),
        ])),
        &path,
    )?;
    let saved = Config::parse(&fs::read_to_string(&path)?)?.remote.relays;
    assert_eq!(
        saved,
        [
            Relay::from("https://sfo3.example.org"),
            Relay {
                // Written as the daemon names it.
                endpoint: "https://fra3.example.org".into(),
                ..fra3.clone()
            },
        ]
    );
    // A label the daemon would refuse is not written.
    let blank = Relay {
        label: RelayLabel {
            name: Some(" ".into()),
            ..RelayLabel::default()
        },
        ..fra3
    };
    let before = fs::read_to_string(&path)?;
    assert!(
        Config::save_preference_path(Preference::Remote(Edit::Relays(vec![blank])), &path).is_err()
    );
    assert_eq!(fs::read_to_string(&path)?, before);
    Ok(())
}

#[test]
fn a_relay_named_twice_keeps_its_place_and_the_label_the_daemon_uses() -> anyhow::Result<()> {
    use crate::config::{preferences::Preference, remote::Edit};
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("local.toml");
    let named = |name: &str| Relay {
        endpoint: "https://fra3.example.org".into(),
        label: RelayLabel {
            name: Some(name.into()),
            ..RelayLabel::default()
        },
    };
    // Plain first, then labelled, then plain again, as a hand-edited list
    // may name it: the label survives, as the daemon would read it.
    Config::save_preference_path(
        Preference::Remote(Edit::Relays(vec![
            "https://fra3.example.org".into(),
            "https://sfo3.example.org".into(),
            named("fra3"),
            "https://fra3.example.org/".into(),
        ])),
        &path,
    )?;
    let saved = Config::parse(&fs::read_to_string(&path)?)?.remote.relays;
    assert_eq!(
        saved,
        [named("fra3"), Relay::from("https://sfo3.example.org")]
    );
    // Of two labels, the later one, as the daemon takes it.
    Config::save_preference_path(
        Preference::Remote(Edit::Relays(vec![named("old"), named("new")])),
        &path,
    )?;
    let saved = Config::parse(&fs::read_to_string(&path)?)?.remote.relays;
    assert_eq!(saved, [named("new")]);
    Ok(())
}
