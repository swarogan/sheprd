//! Desktops let in over SSH through this computer's relays
//! (`docs/relay-plan.md`, Adding a nearby Sheprd from another network).
//!
//! A desktop that paired over Bluetooth, with no network path to this
//! computer, sends an `ssh-ed25519` key with its claim. Once the person here
//! allows it, that key goes into `~/.ssh/authorized_keys` on one line of
//! its own, restricted to connections from loopback: the daemon's relay
//! session is the only way to reach the SSH server from there, so the key
//! cannot be used from the network even where SSH is reachable. The line
//! ends with a comment naming the device, so revoking the device, or its
//! expiring, removes exactly that line and nothing the user wrote.

mod error;

pub use error::{Error, Result};

use crate::control::DeviceId;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

/// The local SSH server every admitted desktop session is spliced to.
pub(crate) const SERVER: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 22);
/// What precedes the device id in the line's comment.
const MARKER: &str = "sheprd-desktop:";
/// Only loopback, which only the daemon's relay sessions reach from afar.
/// Port forwarding stays allowed: Sheprd forwards the ports a remote
/// program listens on.
const OPTIONS: &str = r#"from="127.0.0.1,::1",no-agent-forwarding,no-X11-forwarding"#;
/// The largest `authorized_keys` rewritten; a bigger one is not ours to
/// rewrite.
const MAX_FILE: u64 = 1024 * 1024;
/// The longest public key line accepted from a desktop.
const MAX_KEY: usize = 256;
const ED25519: &str = "ssh-ed25519";
/// Where OpenSSH keeps the host keys.
const HOST_KEY_DIR: &str = "/etc/ssh";
/// Host key files, the ones OpenSSH makes by default, strongest first.
const HOST_KEYS: [&str; 3] = [
    "ssh_host_ed25519_key.pub",
    "ssh_host_ecdsa_key.pub",
    "ssh_host_rsa_key.pub",
];
/// The longest host key line read.
const MAX_HOST_KEY: u64 = 16 * 1024;

/// A desktop's SSH public key: `ssh-ed25519 BASE64`, without a comment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) struct SshKey(String);

impl SshKey {
    /// Accepts `ssh-ed25519 BASE64 [comment]`; the comment is dropped.
    pub(crate) fn parse(text: &str) -> Result<Self> {
        if text.len() > MAX_KEY {
            return Err(Error::InvalidKey);
        }
        let mut fields = text.split_ascii_whitespace();
        let (Some(ED25519), Some(blob)) = (fields.next(), fields.next()) else {
            return Err(Error::InvalidKey);
        };
        let bytes = STANDARD.decode(blob).map_err(|_| Error::InvalidKey)?;
        // string "ssh-ed25519", then string of the 32-byte key.
        let expected = [
            &(ED25519.len() as u32).to_be_bytes()[..],
            ED25519.as_bytes(),
            &32u32.to_be_bytes(),
        ]
        .concat();
        if bytes.len() != expected.len() + 32 || !bytes.starts_with(&expected) {
            return Err(Error::InvalidKey);
        }
        Ok(Self(format!("{ED25519} {blob}")))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SshKey {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::parse(&text)
    }
}

impl From<SshKey> for String {
    fn from(key: SshKey) -> Self {
        key.0
    }
}

/// What a desktop needs to log in once allowed: the user to log in as, and
/// the host keys to pin, read from this computer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Login {
    pub user: String,
    /// `TYPE BASE64` lines, without comments.
    pub host_keys: Vec<String>,
}

impl Login {
    /// This computer's user, and the host keys in `host_key_dir`. Blocking.
    pub(crate) fn here(host_key_dir: &Path) -> Result<Self> {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .ok()
            .filter(|user| valid_user(user))
            .ok_or(Error::UnknownUser)?;
        let host_keys = HOST_KEYS
            .iter()
            .filter_map(|name| read_bounded(&host_key_dir.join(name), MAX_HOST_KEY).ok())
            .filter_map(|text| host_key(&text))
            .collect();
        Ok(Self { user, host_keys })
    }
}

/// A user name `ssh -l` takes as is.
pub(crate) fn valid_user(user: &str) -> bool {
    !user.is_empty()
        && user.len() <= 64
        && !user.starts_with('-')
        && user
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// `TYPE BASE64` from a host key file's first line.
fn host_key(text: &str) -> Option<String> {
    let mut fields = text.lines().next()?.split_ascii_whitespace();
    let (kind, blob) = (fields.next()?, fields.next()?);
    let valid = kind.starts_with("ssh-") || kind.starts_with("ecdsa-");
    (valid && STANDARD.decode(blob).is_ok()).then(|| format!("{kind} {blob}"))
}

/// Where desktops are let in: the `authorized_keys` their keys go into,
/// and the SSH server their relay sessions reach.
#[derive(Clone, Debug)]
pub(crate) struct SshAccess {
    /// Where the SSH server's host keys are, which desktops pin.
    pub(crate) host_key_dir: PathBuf,
    pub(crate) keys: AuthorizedKeys,
    pub(crate) server: SocketAddr,
}

impl SshAccess {
    /// The user's `authorized_keys`, and the SSH server on port 22.
    pub(crate) fn of_user() -> Result<Self> {
        Ok(Self {
            host_key_dir: PathBuf::from(HOST_KEY_DIR),
            keys: AuthorizedKeys::of_user()?,
            server: SERVER,
        })
    }

    /// Whether the SSH server answers. Blocking, for at most a second.
    pub(crate) fn server_listening(&self) -> bool {
        TcpStream::connect_timeout(&self.server, Duration::from_secs(1)).is_ok()
    }
}

/// The `authorized_keys` file desktops are added to.
#[derive(Clone, Debug)]
pub(crate) struct AuthorizedKeys {
    path: PathBuf,
}

impl AuthorizedKeys {
    /// `~/.ssh/authorized_keys` of the user the daemon runs as. Windows
    /// keeps administrators' keys elsewhere, and its OpenSSH server is
    /// rarely on: desktops are not let in there.
    pub(crate) fn of_user() -> Result<Self> {
        if cfg!(windows) {
            return Err(Error::Unsupported);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
            .ok_or(Error::NoHome)?;
        Ok(Self::at(home.join(".ssh").join("authorized_keys")))
    }

    pub(crate) fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// Lets `device` in with `key`, replacing a line it had. Blocking.
    pub(crate) fn allow(&self, device: &DeviceId, key: &SshKey) -> Result<()> {
        let line = format!("{OPTIONS} {} {MARKER}{}", key.as_str(), device.0);
        self.rewrite(|lines| {
            lines.retain(|existing| !ours(existing, device));
            lines.push(line);
        })
    }

    /// Removes `device`'s line; whether it had one. A missing file has none.
    /// Blocking.
    pub(crate) fn remove(&self, device: &DeviceId) -> Result<bool> {
        let mut removed = false;
        if !self.path.exists() {
            return Ok(false);
        }
        self.rewrite(|lines| {
            let before = lines.len();
            lines.retain(|existing| !ours(existing, device));
            removed = lines.len() != before;
        })?;
        Ok(removed)
    }

    /// Removes every line Sheprd wrote for a device `keep` does not keep:
    /// one whose removal failed when it was revoked, or that a restored
    /// device list no longer has. The user's own lines stay. Returns how
    /// many went. Blocking.
    pub(crate) fn sweep(&self, keep: impl Fn(&str) -> bool) -> Result<usize> {
        if !self.path.exists() {
            return Ok(0);
        }
        let mut swept = 0;
        self.rewrite(|lines| {
            let before = lines.len();
            lines.retain(|line| device_of(line).is_none_or(&keep));
            swept = before - lines.len();
        })?;
        Ok(swept)
    }

    /// Reads the file's lines, lets `change` edit them, and replaces the
    /// file atomically, private to this user, only when they changed.
    fn rewrite(&self, change: impl FnOnce(&mut Vec<String>)) -> Result<()> {
        let path = &self.path;
        let file_error = |operation| {
            move |source| Error::File {
                operation,
                path: path.clone(),
                source,
            }
        };
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Linked { path: path.clone() });
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(file_error("read")(error)),
        }
        let text = match read_bounded(path, MAX_FILE) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) if error.kind() == io::ErrorKind::FileTooLarge => {
                return Err(Error::TooLarge { path: path.clone() });
            }
            Err(error) => return Err(file_error("read")(error)),
        };
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        let before = lines.clone();
        change(&mut lines);
        if lines == before {
            return Ok(());
        }
        let directory = path.parent().ok_or(Error::NoHome)?;
        create_private_dir(directory).map_err(file_error("create"))?;
        let mut content = lines.join("\n");
        content.push('\n');
        write_private(directory, path, content.as_bytes()).map_err(file_error("write"))
    }
}

/// Whether `line` is the one Sheprd wrote for `device`.
fn ours(line: &str, device: &DeviceId) -> bool {
    device_of(line).is_some_and(|id| id == device.0)
}

/// The device a line Sheprd wrote names, restricted as Sheprd writes it; a
/// line the user wrote, even one ending in the marker, names none.
fn device_of(line: &str) -> Option<&str> {
    if !line.starts_with(OPTIONS) {
        return None;
    }
    line.split_ascii_whitespace().last()?.strip_prefix(MARKER)
}

/// The file's text, refusing more than `limit` bytes with
/// `ErrorKind::FileTooLarge`.
fn read_bounded(path: &Path, limit: u64) -> io::Result<String> {
    use std::io::Read as _;
    let mut text = String::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    Ok(text)
}

fn create_private_dir(directory: &Path) -> io::Result<()> {
    if directory.is_dir() {
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

/// Writes `content` to a new file in `directory` and moves it over `path`,
/// so sshd never reads a half-written file.
fn write_private(directory: &Path, path: &Path, content: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
