//! Private files in the `desktop` folder: read with a bound, replaced
//! atomically, readable by this user alone.

use super::{Error, Result};
use std::{
    fs,
    io::{self, Read as _, Write as _},
    path::Path,
};

/// The file's bytes, `None` when it is missing; more than `limit` bytes is
/// not a file pairing wrote.
pub(super) fn read_bounded(path: &Path, limit: u64) -> Result<Option<Vec<u8>>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(file_error("read", path, source)),
    };
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| file_error("read", path, source))?;
    if bytes.len() as u64 > limit {
        return Err(Error::RecordFormat);
    }
    Ok(Some(bytes))
}

pub(super) fn create_private_dir(directory: &Path) -> Result<()> {
    if directory.is_dir() {
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(directory)
        .map_err(|source| file_error("create", directory, source))
}

/// Replaces `path` with `content`, private to this user, creating its
/// folder.
pub(super) fn write_private(path: &Path, content: &[u8]) -> Result<()> {
    let directory = path.parent().ok_or(Error::RecordFormat)?;
    create_private_dir(directory)?;
    let write = || -> io::Result<()> {
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            temporary
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        temporary.write_all(content)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    };
    write().map_err(|source| file_error("write", path, source))
}

fn file_error(operation: &'static str, path: &Path, source: io::Error) -> Error {
    Error::File {
        operation,
        path: path.to_owned(),
        source,
    }
}
