use super::*;
use serde_json::json;

fn record(pid: i32, foreground: serde_json::Value) -> Record {
    serde_json::from_value(json!({
        "address": "t1",
        "pid": pid,
        "state": "live",
        "foreground": foreground,
    }))
    .unwrap()
}

#[test]
fn a_shell_at_its_prompt_is_idle() {
    let login = record(10, json!({ "pid": 10, "argv": ["-zsh"], "name": "zsh" }));
    assert_eq!(of(&login), Some(Job::Idle));
    let path = record(10, json!({ "pid": 10, "argv": ["/bin/bash", "-l"] }));
    assert_eq!(of(&path), Some(Job::Idle));
}

#[test]
fn a_job_in_front_of_the_shell_is_its_command_line() {
    let job = record(
        10,
        json!({ "pid": 42, "argv": ["/usr/local/bin/cargo", "test", "-p", "x"] }),
    );
    assert_eq!(
        of(&job),
        Some(Job::Running {
            command: "cargo test -p x".to_owned()
        })
    );
}

#[test]
fn a_terminal_started_with_a_program_runs_it() {
    let direct = record(10, json!({ "pid": 10, "argv": ["htop"] }));
    assert_eq!(
        of(&direct),
        Some(Job::Running {
            command: "htop".to_owned()
        })
    );
}

#[test]
fn the_name_stands_in_for_a_missing_argv() {
    let named = record(10, json!({ "pid": 42, "name": "vim" }));
    assert_eq!(
        of(&named),
        Some(Job::Running {
            command: "vim".to_owned()
        })
    );
}

#[test]
fn an_unknown_foreground_says_nothing() {
    let unknown: Record =
        serde_json::from_value(json!({ "address": "t1", "state": "live" })).unwrap();
    assert_eq!(of(&unknown), None);
    let nameless = record(10, json!({ "pid": 42 }));
    assert_eq!(of(&nameless), None);
}

#[test]
fn a_command_line_is_one_bounded_line() {
    let long = "a".repeat(500);
    let job = record(10, json!({ "pid": 42, "argv": ["echo", "one\ntwo", long] }));
    let Some(Job::Running { command }) = of(&job) else {
        panic!("not running")
    };
    assert_eq!(command.chars().count(), MAX_COMMAND_CHARS);
    assert!(command.starts_with("echo one two "));
    assert!(command.ends_with('…'));
    assert!(!command.contains('\n'));
}
