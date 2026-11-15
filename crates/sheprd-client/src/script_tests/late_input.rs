use super::*;
use std::{sync::mpsc, thread};

/// Input whose end arrives well after the script has exited, so the runner
/// polls the exited child many times before its stdin worker finishes.
struct LateEnd(Duration);

impl Read for LateEnd {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        thread::sleep(self.0);
        Ok(0)
    }
}

#[test]
fn a_script_that_exits_before_its_input_ends_is_still_reaped() {
    #[cfg(unix)]
    let command = {
        let mut command = crate::new_command("/bin/sh");
        command.args(["-c", "exit 0"]);
        command
    };
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt as _;
        let mut command = crate::new_command("cmd.exe");
        command.raw_arg("/D /S /C \"exit 0\"");
        command
    };
    let (done, finished) = mpsc::channel();
    // A thread, so a runner that never returns fails here rather than
    // holding the whole test binary until CI's job limit.
    thread::spawn(move || {
        let result = run_script_command(
            command,
            LateEnd(Duration::from_millis(500)),
            std::io::sink(),
            ScriptLimits {
                output: 1024,
                idle: Duration::from_secs(30),
            },
            &AtomicBool::new(false),
        );
        let _ = done.send(result);
    });
    let result = finished.recv_timeout(Duration::from_secs(20));
    assert!(
        matches!(result, Ok(Ok(0))),
        "the runner never reaped a script that had exited: {result:?}"
    );
}
