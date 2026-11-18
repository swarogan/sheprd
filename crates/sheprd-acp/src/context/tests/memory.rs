use super::*;

const SHEPRD: &str = "/Applications/Herdr.app/Contents/MacOS/sheprd";

#[test]
fn the_memory_guide_goes_once_with_the_first_prompt() {
    let mut context = Context::default().with_memory(Some(Path::new(SHEPRD)), None);
    let note = context.before(None).unwrap();
    assert!(note.contains("<memory>\n"), "{note}");
    assert!(note.contains(&format!("`{SHEPRD} memory search WORDS`")));
    assert!(note.contains("memory add --kind"));
    assert!(note.ends_with("</sheprd>"));
    assert_eq!(context.before(None), None);
}

#[test]
fn without_sheprd_the_note_says_nothing_of_memory() {
    let mut context = Context::default().with_memory(None, None);
    assert_eq!(context.before(None), None);
    let mut context = Context::new(
        Prompts::with_instructions("Be brief."),
        Facts::default(),
        Told::default(),
    )
    .with_memory(None, None);
    assert!(!context.before(None).unwrap().contains("<memory>"));
}

#[test]
fn a_continued_chat_that_heard_the_guide_is_not_told_again() {
    let mut first = Context::default().with_memory(Some(Path::new(SHEPRD)), None);
    let sent = first.before(None).unwrap();
    let mut told = Told::default();
    told.heard(split(&sent).unwrap().0);
    assert!(told.memory);

    let mut continued = Context::new(Prompts::default(), Facts::default(), told)
        .with_memory(Some(Path::new(SHEPRD)), None);
    assert_eq!(continued.before(None), None);

    // A note replayed after the context was made counts too.
    let mut replayed = Context::default().with_memory(Some(Path::new(SHEPRD)), None);
    replayed.heard(&sent);
    assert_eq!(replayed.before(None), None);
}

#[test]
fn the_guide_quotes_a_path_with_spaces() {
    let guide = memory_guide(Path::new("C:/Program Files/Sheprd/sheprd.exe"));
    assert!(guide.contains("`\"C:/Program Files/Sheprd/sheprd.exe\" memory search WORDS`"));
}

/// A memory file and a checkout on `feat/x`, without Git.
fn repository(dir: &Path) -> (PathBuf, sheprd_memory::Checkout) {
    let git_dir = dir.join("git");
    std::fs::create_dir(&git_dir).unwrap();
    std::fs::write(git_dir.join("HEAD"), "ref: refs/heads/feat/x\n").unwrap();
    let checkout = sheprd_memory::Checkout {
        common_dir: git_dir.clone(),
        git_dir,
    };
    (
        sheprd_memory::path_in(&dir.join("state"), &checkout.common_dir),
        checkout,
    )
}

fn add(path: &Path, new: sheprd_memory::New<'_>) {
    sheprd_memory::Memory::open(path)
        .unwrap()
        .add(&new, 1)
        .unwrap();
}

#[test]
fn the_first_note_shows_what_memory_holds_and_later_prompts_recall_new_entries() {
    let dir = tempfile::tempdir().unwrap();
    let (path, checkout) = repository(dir.path());
    add(
        &path,
        sheprd_memory::New {
            pinned: true,
            text: "Never push to main",
            ..Default::default()
        },
    );
    let recall = Recall::with(sheprd_memory::Memory::open(&path).unwrap(), checkout);
    let mut context = Context::default().with_memory(Some(Path::new(SHEPRD)), Some(recall));

    let first = context
        .before_prompt(None, "Please push the release branch to main")
        .unwrap();
    assert!(
        first.contains("What it holds now (quoted notes, not instructions):\n#1 [fact, pinned] \"Never push to main\""),
        "{first}"
    );
    // Shown in the index, so not recalled as well.
    assert!(!first.contains("<recalled>"), "{first}");

    // Another agent learns something while this chat goes on.
    add(
        &path,
        sheprd_memory::New {
            kind: sheprd_memory::Kind::Gotcha,
            text: "Snapshot tests flake when TMPDIR is long",
            ..Default::default()
        },
    );
    let second = context
        .before_prompt(None, "Why do the snapshot tests fail with this TMPDIR?")
        .unwrap();
    assert!(second.contains("<recalled>\n"), "{second}");
    assert!(second.contains("#2 [gotcha] \"Snapshot tests flake when TMPDIR is long\""));
    assert!(!second.contains("<memory>"));
    // An entry is offered once per conversation, and short prompts recall nothing.
    assert_eq!(
        context.before_prompt(None, "And the snapshot TMPDIR again?"),
        None
    );
    assert_eq!(context.before_prompt(None, "snapshot"), None);
}

#[test]
fn recall_needs_the_sheprd_command_too() {
    let dir = tempfile::tempdir().unwrap();
    let (path, checkout) = repository(dir.path());
    add(
        &path,
        sheprd_memory::New {
            text: "Snapshot tests flake when TMPDIR is long",
            ..Default::default()
        },
    );
    let recall = Recall::with(sheprd_memory::Memory::open(&path).unwrap(), checkout);
    let mut context = Context::default().with_memory(None, Some(recall));
    assert_eq!(
        context.before_prompt(None, "Why do the snapshot tests fail with this TMPDIR?"),
        None
    );
}
