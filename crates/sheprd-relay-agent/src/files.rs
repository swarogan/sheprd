//! The agent's state directory and the private files in it: written whole
//! (a temporary file renamed over the old one), readable by this user only,
//! and read with a size bound.

use crate::{Error, Result};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const NAME: &str = "sheprd-relay-agent";

/// Where the agent keeps its state when `--state-dir` is not given:
/// `~/Library/Application Support/sheprd-relay-agent` on macOS,
/// `$XDG_STATE_HOME/sheprd-relay-agent` (or `~/.local/state/…`) elsewhere.
pub fn default_state_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from);
    if cfg!(target_os = "macos") {
        let home = home.ok_or(Error::NoHome)?;
        return Ok(home.join("Library/Application Support").join(NAME));
    }
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    match (state, home) {
        (Some(state), _) => Ok(state.join(NAME)),
        (None, Some(home)) => Ok(home.join(".local/state").join(NAME)),
        (None, None) => Err(Error::NoHome),
    }
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// Replaces `path` with `content`, private to this user, creating its
/// directory (also private) when missing. Blocking.
pub fn write_private(path: &Path, content: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let parent = path.parent().unwrap_or(Path::new("."));
    create_private_dir(parent)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = parent.join(format!(
        ".{name}-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let written = options
        .open(&temp)
        .and_then(|mut file| {
            file.write_all(content)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written.map_err(io_error(path))
}

fn create_private_dir(dir: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(io_error(dir))
}

/// `path`'s bytes, or `None` when it does not exist. A file larger than
/// `limit` is refused rather than read. Blocking.
pub fn read_bounded(path: &Path, limit: u64) -> Result<Option<Vec<u8>>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path)(source)),
    };
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error(path))?;
    if bytes.len() as u64 > limit {
        return Err(Error::TooLarge {
            limit,
            path: path.to_owned(),
        });
    }
    Ok(Some(bytes))
}

/// An exclusive lock on the state directory, held until dropped. Every
/// read-change-write of the agent's files takes it, so two commands (or a
/// command and the running agent) never both make keys, and an
/// `authorize` never writes back a desktop a `revoke` just removed.
#[derive(Debug)]
pub struct Lock {
    _file: fs::File,
}

const LOCK: &str = "lock";

impl Lock {
    /// Waits for the lock on `state_dir`, creating the directory and its
    /// lock file when missing. Blocking.
    pub fn acquire(state_dir: &Path) -> Result<Self> {
        let (file, path) = lock_file(state_dir, LOCK)?;
        file.lock().map_err(io_error(&path))?;
        Ok(Self { _file: file })
    }
}

/// Held by the running agent for its whole life, so a second `run` on the
/// same state directory refuses to start rather than taking the route from
/// the first on every relay, which would leave the first listening on
/// nothing.
#[derive(Debug)]
pub struct RunLock {
    _file: fs::File,
}

const RUN_LOCK: &str = "run.lock";

impl RunLock {
    /// Takes the run lock on `state_dir`, or [`Error::AlreadyRunning`].
    /// Blocking only to open the file.
    pub fn try_acquire(state_dir: &Path) -> Result<Self> {
        let (file, path) = lock_file(state_dir, RUN_LOCK)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(fs::TryLockError::WouldBlock) => Err(Error::AlreadyRunning { path }),
            Err(fs::TryLockError::Error(source)) => Err(io_error(&path)(source)),
        }
    }
}

fn lock_file(state_dir: &Path, name: &str) -> Result<(fs::File, PathBuf)> {
    create_private_dir(state_dir)?;
    let path = state_dir.join(name);
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path).map_err(io_error(&path))?;
    Ok((file, path))
}

/// Removes `path`, when it exists. Blocking.
pub fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(io_error(path)(error)),
        _ => Ok(()),
    }
}
