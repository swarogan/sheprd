//! Finding the T3 Code server running on this computer.
//!
//! A running server writes where it listens to `server-runtime.json` in its
//! data directory: `$T3CODE_HOME`, or `~/.t3`, under `userdata`. The file
//! is only a hint; [`crate::http::Server::describe`] proves a server answers.

use std::{
    ffi::OsStr,
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use url::Url;

use crate::{Error, Result};

/// The most the runtime file may weigh.
const LIMIT: u64 = 64 * 1024;

#[derive(Deserialize)]
struct Runtime {
    origin: String,
}

/// T3 Code's data directory, from `T3CODE_HOME` (`home_var`) or `home`.
pub fn t3_home(home_var: Option<&OsStr>, home: Option<&Path>) -> Option<PathBuf> {
    match home_var.filter(|value| !value.is_empty()) {
        Some(value) => {
            let value = Path::new(value);
            match value.strip_prefix("~") {
                Ok(rest) => home.map(|home| home.join(rest)),
                Err(_) => Some(value.to_path_buf()),
            }
        }
        None => home.map(|home| home.join(".t3")),
    }
}

/// The origin of the server whose data directory is `t3_home`.
pub fn local_origin(t3_home: &Path) -> Result<Url> {
    let path = t3_home.join("userdata").join("server-runtime.json");
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NoServer);
        }
        Err(source) => return Err(Error::Discovery { path, source }),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Discovery {
            path: path.clone(),
            source,
        })?;
    if bytes.len() as u64 > LIMIT {
        return Err(Error::TooLarge {
            limit: LIMIT as usize,
        });
    }
    let runtime: Runtime = serde_json::from_slice(&bytes)?;
    let origin = Url::parse(&runtime.origin)?;
    // Only a server on this computer is found this way.
    match origin.host_str() {
        Some("127.0.0.1" | "localhost" | "[::1]" | "::1") => Ok(origin),
        _ => Err(Error::NoServer),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
