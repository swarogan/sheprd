//! An access token kept in a file the user names, bound to the server that
//! issued it.
//!
//! The file holds the server's environment id beside the token, and
//! [`load`] refuses the token for any other server: a process that takes
//! over the discovered port, or a mistyped `--server`, never receives it.
//! [`save`] writes a new file readable only by its owner and renames it into
//! place, so neither an existing file's looser mode nor a symbolic link at
//! the path is inherited.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::Path,
};

use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};

mod error;
pub use error::Error;

/// The most a token file may weigh.
const LIMIT: u64 = 16 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    environment_id: String,
    token: SecretString,
}

/// Keeps `token`, issued by the server `environment_id`, in `path`.
pub fn save(path: &Path, environment_id: &str, token: &SecretString) -> Result<(), Error> {
    let write = |source| Error::Write {
        path: path.to_owned(),
        source,
    };
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Saving<'a> {
        environment_id: &'a str,
        token: &'a str,
    }
    let text = serde_json::to_vec(&Saving {
        environment_id,
        token: token.expose_secret(),
    })
    .map_err(|error| write(std::io::Error::other(error)))?;
    let name = path
        .file_name()
        .ok_or_else(|| write(std::io::ErrorKind::InvalidInput.into()))?;
    let mut staged = name.to_owned();
    staged.push(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let staged = path.with_file_name(staged);
    let mut options = OpenOptions::new();
    // A new file only: never one another process prepared at that name.
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let result = options
        .open(&staged)
        .and_then(|mut file| file.write_all(&text).and_then(|()| file.sync_all()))
        .and_then(|()| fs::rename(&staged, path));
    if result.is_err() {
        let _ = fs::remove_file(&staged);
    }
    result.map_err(write)
}

/// The token in `path`, if the server `environment_id` issued it.
pub fn load(path: &Path, environment_id: &str) -> Result<SecretString, Error> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(LIMIT + 1).read_to_end(&mut bytes))
        .map_err(|source| Error::Read {
            path: path.to_owned(),
            source,
        })?;
    let invalid = || Error::Invalid {
        path: path.to_owned(),
    };
    if bytes.len() as u64 > LIMIT {
        return Err(invalid());
    }
    let saved: Saved = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if saved.environment_id != environment_id {
        return Err(Error::OtherServer {
            path: path.to_owned(),
        });
    }
    if saved.token.expose_secret().trim().is_empty() {
        return Err(invalid());
    }
    Ok(saved.token)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
