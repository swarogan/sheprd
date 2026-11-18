use std::{ffi::OsString, fs};

use super::*;

#[test]
fn the_data_directory_follows_t3code_home_then_home() {
    let home = Path::new("/home/me");
    assert_eq!(t3_home(None, Some(home)), Some(home.join(".t3")));
    let empty = OsString::new();
    assert_eq!(t3_home(Some(&empty), Some(home)), Some(home.join(".t3")));
    let set = OsString::from("/srv/t3");
    assert_eq!(t3_home(Some(&set), None), Some(PathBuf::from("/srv/t3")));
    let tilde = OsString::from("~/other");
    assert_eq!(t3_home(Some(&tilde), Some(home)), Some(home.join("other")));
    assert_eq!(t3_home(None, None), None);
}

fn runtime(origin: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("userdata")).unwrap();
    fs::write(
        dir.path().join("userdata/server-runtime.json"),
        serde_json::json!({"version": 1, "pid": 1, "host": "127.0.0.1", "port": 3773,
            "origin": origin})
        .to_string(),
    )
    .unwrap();
    dir
}

#[test]
fn a_local_server_is_found_by_its_runtime_file() {
    let dir = runtime("http://127.0.0.1:47731");
    assert_eq!(
        local_origin(dir.path()).unwrap().as_str(),
        "http://127.0.0.1:47731/"
    );
}

#[test]
fn no_file_another_host_or_an_oversized_file_finds_nothing() {
    let empty = tempfile::tempdir().unwrap();
    assert!(matches!(local_origin(empty.path()), Err(Error::NoServer)));
    let remote = runtime("http://10.0.0.5:3773");
    assert!(matches!(local_origin(remote.path()), Err(Error::NoServer)));
    let big = tempfile::tempdir().unwrap();
    fs::create_dir(big.path().join("userdata")).unwrap();
    fs::write(
        big.path().join("userdata/server-runtime.json"),
        " ".repeat(LIMIT as usize + 1),
    )
    .unwrap();
    assert!(matches!(
        local_origin(big.path()),
        Err(Error::TooLarge { .. })
    ));
}
