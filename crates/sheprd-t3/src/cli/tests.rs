use super::*;

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn each_command_parses_with_its_options() {
    assert_eq!(
        parse(args(&["pair", "--token-file", "/t"])).unwrap(),
        Options {
            command: Command::Pair,
            json: false,
            server: None,
            token_file: Some(PathBuf::from("/t")),
        }
    );
    let threads = parse(args(&[
        "threads",
        "--json",
        "--server",
        "http://h:1",
        "--token-file",
        "/t",
    ]))
    .unwrap();
    assert_eq!(threads.command, Command::Threads);
    assert!(threads.json);
    assert_eq!(threads.server.as_deref(), Some("http://h:1"));
    assert_eq!(
        parse(args(&["show", "mcp:1", "--token-file", "/t"]))
            .unwrap()
            .command,
        Command::Show {
            thread: "mcp:1".to_owned()
        }
    );
    assert_eq!(parse(args(&["--help"])).unwrap().command, Command::Help);
}

#[test]
fn mistakes_are_refused_before_anything_runs() {
    assert_eq!(parse(args(&[])), Err(Error::NoCommand));
    assert_eq!(parse(args(&["threads"])), Err(Error::TokenFile));
    assert_eq!(
        parse(args(&["show", "--token-file", "/t"])),
        Err(Error::Arguments("show"))
    );
    assert_eq!(
        parse(args(&["threads", "extra", "--token-file", "/t"])),
        Err(Error::Arguments("threads"))
    );
    assert_eq!(
        parse(args(&["pair", "--server"])),
        Err(Error::Missing("--server"))
    );
    // A pairing code in the arguments would be visible to other users.
    assert_eq!(
        parse(args(&["pair", "CODE", "--token-file", "/t"])),
        Err(Error::Arguments("pair"))
    );
    assert_eq!(
        parse(args(&["threads", "--verbose"])),
        Err(Error::Unknown("--verbose".to_owned()))
    );
    assert_eq!(
        parse(args(&["delete", "--token-file", "/t"])),
        Err(Error::Unknown("delete".to_owned()))
    );
}

#[test]
fn server_text_cannot_drive_the_terminal() {
    assert_eq!(
        plain("Fix \u{1b}]52;c;cm0gLXJm\u{7}it"),
        "Fix \u{fffd}]52;c;cm0gLXJm\u{fffd}it"
    );
    assert_eq!(plain("a\tb\nc\u{9b}d"), "a\u{fffd}b\u{fffd}c\u{fffd}d");
    assert_eq!(plain("é 日本"), "é 日本");
}
