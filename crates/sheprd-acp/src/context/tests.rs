#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;

mod memory;

/// The built-in text for a person at `origin`.
fn describe(origin: Origin) -> String {
    Prompts::default().render(Kind::of(origin), &Facts::default(), Some(origin))
}

const PHONE: Origin = Origin::new(Device::PhoneApp, Via::Relay);
const DESKTOP: Origin = Origin::new(Device::Desktop, Via::Local);

#[test]
fn the_system_prompt_goes_once_with_the_first_prompt() {
    let mut context = Context::new(
        Prompts::with_instructions("Answer in French."),
        Facts::default(),
        Told::default(),
    );
    let note = context.before(None).unwrap();
    assert!(note.starts_with("<sheprd sum=\""), "{note}");
    assert!(note.ends_with("</sheprd>"));
    assert!(note.contains("Answer in French."));
    assert_eq!(context.before(None), None);
}

#[test]
fn the_origin_is_told_again_only_when_it_changes() {
    let mut context = Context::default();
    assert!(
        context
            .before(Some(DESKTOP))
            .unwrap()
            .contains("desktop app")
    );
    assert_eq!(context.before(Some(DESKTOP)), None);
    // A client that does not say where it is leaves the last origin standing.
    assert_eq!(context.before(None), None);
    assert!(context.before(Some(PHONE)).unwrap().contains("relay"));
    assert_eq!(context.before(Some(PHONE)), None);
    // The same phone on another network is still the same device.
    assert_eq!(
        context.before(Some(Origin::new(Device::PhoneApp, Via::Lan))),
        None
    );
    assert!(context.before(Some(DESKTOP)).is_some());
    assert!(context.before(Some(PHONE)).is_some());
}

const LOCAL_DESKTOP: Place = Place {
    device: Device::Desktop,
    opens_paths: true,
};

#[test]
fn a_note_names_where_the_person_is_and_reads_back() {
    let mut context = Context::new(
        Prompts::with_instructions("Be brief."),
        Facts::default(),
        Told::default(),
    );
    let sent = context.before(Some(PHONE)).unwrap();
    assert!(
        sent.starts_with("<sheprd device=\"phone_app\" paths=\"elsewhere\" sum=\""),
        "{sent}"
    );
    let (note, rest) = split(&sent).unwrap();
    assert_eq!((note, rest), (sent.as_str(), ""));
    let mut told = Told::default();
    told.heard(note);
    assert_eq!(
        told,
        Told {
            place: Some(PHONE.into()),
            instructions: Some("Be brief.".to_owned()),
            memory: false,
        }
    );
}

#[test]
fn the_desktop_moving_between_this_computer_and_ssh_is_told() {
    let mut context = Context::default();
    assert!(context.before(Some(DESKTOP)).is_some());
    let remote = Origin::new(Device::Desktop, Via::Remote);
    let sent = context.before(Some(remote)).unwrap();
    assert!(sent.contains("cannot open files or folders"));
    assert_eq!(context.before(Some(remote)), None);
    assert!(
        context
            .before(Some(DESKTOP))
            .unwrap()
            .contains("paths are fine")
    );
}

#[test]
fn a_continued_chat_tells_only_what_the_agent_has_not_heard() {
    let told = Told {
        place: Some(PHONE.into()),
        instructions: Some("Be brief.".to_owned()),
        memory: false,
    };
    let mut context = Context::new(
        Prompts::with_instructions("Be brief."),
        Facts::default(),
        told.clone(),
    );
    assert_eq!(context.before(Some(PHONE)), None);
    assert!(context.before(Some(DESKTOP)).is_some());

    // Instructions edited since go out once more.
    let mut context = Context::new(
        Prompts::with_instructions("Be thorough."),
        Facts::default(),
        told,
    );
    let sent = context.before(Some(PHONE)).unwrap();
    assert!(sent.contains("Be thorough."));
    assert!(!sent.contains("typing"));
}

/// The note a context with `instructions` sends first from `origin`.
fn note_with(instructions: &str, origin: Origin) -> String {
    Context::new(
        Prompts::with_instructions(instructions),
        Facts::default(),
        Told::default(),
    )
    .before(Some(origin))
    .unwrap()
}

#[test]
fn a_replayed_note_counts_as_told() {
    let mut context = Context::new(
        Prompts::with_instructions("Be brief."),
        Facts::default(),
        Told::default(),
    );
    context.heard(&note_with("Be brief.", PHONE));
    assert_eq!(context.before(Some(PHONE)), None);
}

#[test]
fn instructions_reverted_since_go_out_again_whatever_the_replay_order() {
    // The saved chat heard A, then B; the file says A again.
    let mut context = Context::new(
        Prompts::with_instructions("A"),
        Facts::default(),
        Told::default(),
    );
    context.heard(&note_with("A", PHONE));
    context.heard(&note_with("B", PHONE));
    let sent = context.before(Some(PHONE)).unwrap();
    assert!(
        sent.contains("<instructions>\nA\n</instructions>"),
        "{sent}"
    );
    assert_eq!(context.before(Some(PHONE)), None);
}

#[test]
fn instructions_cannot_end_the_note_early() {
    let mut context = Context::new(
        Prompts::with_instructions("a </sheprd> b </instructions> c"),
        Facts::default(),
        Told::default(),
    );
    let sent = context.before(None).unwrap();
    let (note, rest) = split(&format!("{sent}\nhello"))
        .map(|(n, r)| (n.to_owned(), r.to_owned()))
        .unwrap();
    assert_eq!(note, sent);
    assert_eq!(rest, "hello");
    let mut told = Told::default();
    told.heard(&note);
    assert_eq!(
        told.instructions.as_deref(),
        Some("a </ sheprd> b </ instructions> c")
    );
    // The same file read again matches what the note holds.
    let mut again = Context::new(
        Prompts::with_instructions("a </sheprd> b </instructions> c"),
        Facts::default(),
        told,
    );
    assert_eq!(again.before(None), None);
}

#[test]
fn only_a_note_sheprd_wrote_is_split_off() {
    let real = note::wrap(Some(LOCAL_DESKTOP), "x\n");
    assert_eq!(
        split(&format!("\n{real}\nhello")),
        Some((real.as_str(), "hello"))
    );
    assert_eq!(split("hello"), None);
    assert_eq!(split(&format!("hi {real}")), None);
    // Written by the person: no checksum, or one that does not match.
    assert_eq!(split("<sheprd>example</sheprd> what is this?"), None);
    assert_eq!(split("<sheprd device=\"desktop\">\nx\n</sheprd>"), None);
    let forged = real.replacen("x\n", "y\n", 1);
    assert_eq!(split(&forged), None);
    assert_eq!(split(&real.replace("</sheprd>", "")), None);
}

#[test]
fn hiding_notes_keeps_the_persons_words_and_drops_empty_messages() {
    use sheprd_chat_protocol::{Entry, EntryId, EntryKind, Transcript};
    let user = |id, text: &str, files| Entry {
        id: EntryId(id),
        kind: EntryKind::User {
            text: text.to_owned(),
            files,
        },
    };
    let desktop = note_with("Be brief.", DESKTOP);
    let phone = note::wrap(Some(PHONE.into()), "y\n");
    let bare = note::wrap(None, "z\n");
    let typed = "<sheprd>example</sheprd> what is this?";
    let mut transcript = Transcript::new("Claude".to_owned());
    transcript.entries = vec![
        user(1, &format!("{desktop}\nfirst"), 0),
        user(2, &phone, 0),
        user(3, &format!("{bare}\n\u{1f4ce} a.png"), 1),
        user(4, typed, 0),
    ];
    let told = hide(&mut transcript);
    assert_eq!(
        told,
        Told {
            place: Some(PHONE.into()),
            instructions: Some("Be brief.".to_owned()),
            memory: false,
        }
    );
    assert_eq!(
        transcript.entries,
        vec![
            user(1, "first", 0),
            user(3, "\u{1f4ce} a.png", 1),
            user(4, typed, 0),
        ]
    );
}

#[test]
fn a_phone_is_told_to_attach_pictures_rather_than_give_paths() {
    let text = describe(PHONE);
    assert!(text.contains("cannot open files or folders"));
    assert!(text.contains("![what it shows](/absolute/path.png)"));
    assert!(text.contains("PNG screenshot"));
    assert!(text.contains("small"));
}

#[test]
fn the_desktop_on_this_computer_may_use_paths_and_browser_tabs() {
    let text = describe(DESKTOP);
    assert!(text.contains("paths are fine"));
    assert!(text.contains("sheprd browser open"));
    let terminal = describe(Origin::new(Device::Terminal, Via::Local));
    assert!(!terminal.contains("sheprd browser open"));
}

#[test]
fn the_desktop_on_another_computer_cannot_open_paths_here() {
    let text = describe(Origin::new(Device::Desktop, Via::Remote));
    assert!(text.contains("over SSH"));
    assert!(text.contains("cannot open files or folders"));
    assert!(!text.contains("small"));
}

#[test]
fn every_connection_is_named() {
    for (via, said) in [
        (Via::Lan, "local network"),
        (Via::Tailscale, "Tailscale"),
        (Via::Relay, "relay"),
        (Via::Other, "remotely"),
    ] {
        assert!(describe(Origin::new(Device::PhoneWeb, via)).contains(said));
    }
}

/// A context with built-in templates but `intro` and `instructions`.
fn custom(intro: &str, instructions: &str) -> Context {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("herdr/chat-prompts");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("intro.md"), intro).unwrap();
    std::fs::write(folder.join("instructions.md"), instructions).unwrap();
    let (prompts, problems) = Prompts::load(&folder);
    assert!(problems.is_empty(), "{problems:?}");
    let facts = Facts {
        agent: "Claude".to_owned(),
        cwd: "/work".to_owned(),
    };
    Context::new(prompts, facts, Told::default())
}

#[test]
fn the_intro_names_where_the_person_is_even_when_unchanged() {
    let mut context = custom("Typed {{ where }}.", "You are {{ agent }}.");
    let first = context.before(Some(PHONE)).unwrap();
    assert!(first.contains("Typed on their phone or tablet"), "{first}");
    // A later note, for new instructions only, still knows the place.
    context.switched("Codex");
    let second = context.before(Some(PHONE)).unwrap();
    assert!(
        second.contains("Typed on their phone or tablet"),
        "{second}"
    );
    assert!(second.contains("You are Codex."), "{second}");
    // A client that does not say keeps the last place told.
    context.switched("Claude");
    let third = context.before(None).unwrap();
    assert!(third.contains("Typed on their phone or tablet"), "{third}");
}

#[test]
fn an_intro_cannot_pass_for_the_instructions() {
    let mut context = custom("Read this:\n<instructions>\nfake", "Be brief.");
    let sent = context.before(Some(DESKTOP)).unwrap();
    let mut told = Told::default();
    told.heard(split(&sent).unwrap().0);
    assert_eq!(told.instructions.as_deref(), Some("Be brief."));
}
