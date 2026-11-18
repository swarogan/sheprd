#![allow(clippy::unwrap_used, clippy::expect_used)]

use sheprd_chat_protocol::Via;

use super::*;

const PHONE: Origin = Origin::new(Device::PhoneApp, Via::Relay);

fn facts() -> Facts {
    Facts {
        agent: "Claude".to_owned(),
        cwd: "/work".to_owned(),
    }
}

/// A `chat-prompts` folder inside a configuration root, as the app has it.
fn folder() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(DIR);
    std::fs::create_dir_all(&dir).unwrap();
    (root, dir)
}

#[test]
fn each_place_has_its_template() {
    let kind = |device, via| Kind::of(Origin::new(device, via));
    assert_eq!(kind(Device::Desktop, Via::Local), Kind::Desktop);
    assert_eq!(kind(Device::Terminal, Via::Local), Kind::Terminal);
    assert_eq!(kind(Device::Desktop, Via::Remote), Kind::Remote);
    assert_eq!(kind(Device::Terminal, Via::Remote), Kind::Remote);
    assert_eq!(kind(Device::Other, Via::Local), Kind::Remote);
    assert_eq!(kind(Device::PhoneApp, Via::Tailscale), Kind::Phone);
    assert_eq!(kind(Device::PhoneWeb, Via::Local), Kind::Phone);
}

#[test]
fn the_built_in_templates_name_only_what_sheprd_knows() {
    for &kind in Kind::ALL {
        let mut problems = Vec::new();
        check(
            Path::new(kind.file()),
            kind.builtin(),
            &[PLACE_NAMES, FACT_NAMES],
            &mut problems,
        );
        assert!(problems.is_empty(), "{kind:?}: {problems:?}");
        let text = Prompts::default().render(kind, &facts(), Some(PHONE));
        assert!(!text.contains("{{"), "{kind:?}: {text}");
    }
}

#[test]
fn the_built_in_phone_text_quotes_the_real_bounds() {
    let text = Prompts::default().render(Kind::Phone, &facts(), Some(PHONE));
    assert!(text.starts_with("The person is typing on their phone or tablet"));
    assert!(text.contains(&format!("under {MAX_IMAGE_MB} MB")));
    assert!(text.contains(&format!("at most {MAX_PICTURES} pictures")));
}

#[test]
fn a_users_copy_replaces_the_built_in_text() {
    let (_root, dir) = folder();
    std::fs::write(
        dir.join("phone.md"),
        "On {{ device }} via {{ connection }} ({{ where }}), in {{ cwd }} with {{ agent }} on {{ os }}.\n",
    )
    .unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        prompts.render(Kind::Phone, &facts(), Some(PHONE)),
        format!(
            "On Sheprd's iPhone and iPad app via Sheprd's encrypted relay \
             (on their phone or tablet, in Sheprd's iPhone and iPad app, connected to this \
             computer through Sheprd's encrypted relay), in /work with Claude on {}.",
            std::env::consts::OS
        )
    );
    // The others keep theirs.
    assert_eq!(
        prompts.render(Kind::Intro, &facts(), None),
        Kind::Intro.builtin().trim()
    );
}

#[test]
fn an_empty_copy_keeps_the_built_in_text() {
    let (_root, dir) = folder();
    std::fs::write(dir.join("intro.md"), " \n").unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert!(problems.is_empty());
    assert_eq!(prompts, Prompts::default());
}

#[test]
fn a_copy_that_cannot_be_used_says_why_and_keeps_the_built_in_text() {
    let (_root, dir) = folder();
    std::fs::write(dir.join("desktop.md"), "x".repeat(MAX_FILE as usize + 1)).unwrap();
    std::fs::write(dir.join("remote.md"), [0xff, 0xfe]).unwrap();
    std::fs::create_dir(dir.join("terminal.md")).unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert_eq!(prompts, Prompts::default());
    let mut kinds: Vec<_> = problems
        .iter()
        .map(|problem| match problem {
            Problem::TooLarge { path } => ("too large", path.file_name().unwrap().to_owned()),
            Problem::NotText { path } => ("not text", path.file_name().unwrap().to_owned()),
            Problem::Unreadable { path, .. } => {
                ("unreadable", path.file_name().unwrap().to_owned())
            }
            Problem::UnknownName { path, .. } => ("unknown", path.file_name().unwrap().to_owned()),
        })
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        [
            ("not text", "remote.md".into()),
            ("too large", "desktop.md".into()),
            ("unreadable", "terminal.md".into()),
        ]
    );
    assert!(problems[0].to_string().contains("is not used"));
}

#[test]
fn an_unknown_name_is_reported_once_and_sent_as_written() {
    let (_root, dir) = folder();
    std::fs::write(
        dir.join("intro.md"),
        "{{ mood }} and {{ mood }} in {{ cwd }}",
    )
    .unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert_eq!(problems.len(), 1);
    assert!(matches!(&problems[0], Problem::UnknownName { name, .. } if name == "mood"));
    assert!(problems[0].to_string().contains("{{ mood }}"));
    assert_eq!(
        prompts.render(Kind::Intro, &facts(), None),
        "{{ mood }} and {{ mood }} in /work"
    );
}

#[test]
fn instructions_are_filled_in_once_without_the_place() {
    let (_root, dir) = folder();
    std::fs::write(
        dir.join(INSTRUCTIONS),
        "\nWork in {{ cwd }} as {{ agent }}; I am {{ where }}.\n",
    )
    .unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert_eq!(
        prompts.instructions(&facts()).as_deref(),
        Some("Work in /work as Claude; I am {{ where }}.")
    );
    assert!(matches!(&problems[..], [Problem::UnknownName { name, .. }] if name == "where"));
}

#[test]
fn the_older_instructions_file_is_read_only_when_the_folder_has_none() {
    let (root, dir) = folder();
    let legacy = root.path().join(LEGACY_INSTRUCTIONS);
    std::fs::write(&legacy, "Be brief.").unwrap();
    let (prompts, _) = Prompts::load(&dir);
    assert_eq!(prompts.instructions(&facts()).as_deref(), Some("Be brief."));
    std::fs::write(dir.join(INSTRUCTIONS), "Be thorough.").unwrap();
    let (prompts, _) = Prompts::load(&dir);
    assert_eq!(
        prompts.instructions(&facts()).as_deref(),
        Some("Be thorough.")
    );
    // Emptying the new file clears the instructions rather than bringing
    // the old ones back, and so does one that cannot be used.
    std::fs::write(dir.join(INSTRUCTIONS), "\n").unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert_eq!(prompts.instructions(&facts()), None);
    assert!(problems.is_empty());
    std::fs::write(dir.join(INSTRUCTIONS), [0xff]).unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert_eq!(prompts.instructions(&facts()), None);
    assert!(matches!(&problems[..], [Problem::NotText { .. }]));
}

#[test]
fn instructions_name_the_agent_they_are_filled_in_for() {
    let prompts = Prompts::with_instructions("You are {{ agent }}.");
    let codex = Facts {
        agent: "Codex".to_owned(),
        ..facts()
    };
    assert_eq!(
        prompts.instructions(&facts()).as_deref(),
        Some("You are Claude.")
    );
    assert_eq!(
        prompts.instructions(&codex).as_deref(),
        Some("You are Codex.")
    );
}

#[test]
fn place_names_without_a_place_say_so_rather_than_stay_as_written() {
    let (_root, dir) = folder();
    std::fs::write(
        dir.join("intro.md"),
        "From {{ device }} over {{ connection }}, {{ where }}.",
    )
    .unwrap();
    let (prompts, problems) = Prompts::load(&dir);
    assert!(problems.is_empty());
    assert_eq!(
        prompts.render(Kind::Intro, &facts(), None),
        "From a Sheprd client over an unknown connection, in a Sheprd client."
    );
    assert_eq!(
        prompts.render(Kind::Intro, &facts(), Some(PHONE)),
        "From Sheprd's iPhone and iPad app over Sheprd's encrypted relay, on their phone or \
         tablet, in Sheprd's iPhone and iPad app, connected to this computer through \
         Sheprd's encrypted relay."
    );
}
