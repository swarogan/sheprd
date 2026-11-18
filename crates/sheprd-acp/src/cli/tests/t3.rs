use super::*;

#[test]
fn t3_follows_a_thread_with_a_token_file() {
    let parsed = parse(
        [
            "t3",
            "mcp:abc",
            "--token-file",
            "/t",
            "--server",
            "http://127.0.0.1:3773",
            "--pane",
            "p1",
        ]
        .map(OsString::from),
    )
    .unwrap();
    assert_eq!(
        parsed,
        Parsed::T3(T3Options {
            endpoint: None,
            pane: Some("p1".to_owned()),
            server: Some("http://127.0.0.1:3773".to_owned()),
            thread: "mcp:abc".to_owned(),
            token_file: PathBuf::from("/t"),
        })
    );
    assert_eq!(
        parse(["t3", "mcp:abc"].map(OsString::from)),
        Err(Error::MissingValue("--token-file"))
    );
    assert_eq!(
        parse(["t3"].map(OsString::from)),
        Err(Error::MissingValue("t3 THREAD"))
    );
}
