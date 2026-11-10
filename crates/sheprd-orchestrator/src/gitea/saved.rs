//! Gitea instances and their access tokens, pasted in the account menu: the
//! address typed and its token, one line per instance. They are kept together
//! in one credential-store record of `hostname<TAB>base_url<TAB>token`
//! lines, in the desktop keyring, or the private `gitea-credentials` file
//! when `[gitea] allow_plaintext_credentials` opts in. Every read and write
//! blocks on the store, so callers run on a worker.

use super::Error;
use secrecy::{ExposeSecret, SecretString};
use sheprd_credentials::{Entry, Store};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};
use zeroize::Zeroizing;

/// The most instances one record keeps, which also bounds its size.
pub const MAX_INSTANCES: usize = 32;
/// A DNS name is at most 253 characters.
const HOST_LIMIT: usize = 253;
/// The longest address kept, scheme and sub-path included.
const ADDRESS_LIMIT: usize = 512;
/// The longest token kept; Gitea's are 40 hexadecimal characters.
pub const TOKEN_LIMIT: usize = 1024;
const _: () = assert!(
    MAX_INSTANCES * (HOST_LIMIT + ADDRESS_LIMIT + TOKEN_LIMIT + 3) <= sheprd_credentials::LIMIT
);

const ENTRY: Entry<Error> = Entry {
    service: "dev.herdr.gpui.gitea",
    account: "tokens",
    label: "Sheprd Gitea tokens",
    file: c"gitea-credentials",
    validate: |record| Saved::parse(record).map(drop),
};

/// Every read-modify-write of the record, so windows saving at once take
/// turns rather than losing each other's instances.
static RECORD: Mutex<()> = Mutex::new(());

/// One Gitea instance: where it is and the token for it.
#[derive(Clone, Debug)]
pub struct Instance {
    base_url: String,
    token: SecretString,
}

impl Instance {
    /// The address typed, normalized: scheme, host, port, and sub-path,
    /// with no trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn token(&self) -> &SecretString {
        &self.token
    }
}

/// The kept instances, by lowercase hostname without port.
#[derive(Clone, Debug, Default)]
pub struct Saved(BTreeMap<String, Instance>);

impl Saved {
    /// The instance kept for a remote's `host`, whose port is ignored.
    pub fn get(&self, host: &str) -> Option<&Instance> {
        self.0.get(&hostname(host))
    }

    /// The instances kept, sorted by hostname.
    pub fn instances(&self) -> impl Iterator<Item = (&str, &Instance)> {
        self.0
            .iter()
            .map(|(host, instance)| (host.as_str(), instance))
    }

    /// Keeps `token` for the instance at `address`, or forgets that
    /// instance's with `None`; a bare hostname names it as well.
    pub fn set(&mut self, address: &str, token: Option<SecretString>) -> Result<(), Error> {
        let (host, base_url) =
            normalize_address(address).ok_or_else(|| Error::Address(address.to_owned()))?;
        match token {
            Some(token) => {
                let token = checked_token(&token)?;
                if !self.0.contains_key(&host) && self.0.len() >= MAX_INSTANCES {
                    return Err(Error::TooManyInstances);
                }
                self.0.insert(host, Instance { base_url, token });
            }
            None => {
                self.0.remove(&host);
            }
        }
        Ok(())
    }

    /// Reads a record, refusing one that is not lines of a hostname, the
    /// normalized address it belongs to, and a valid token, or that names
    /// more than [`MAX_INSTANCES`].
    pub fn parse(record: &SecretString) -> Result<Self, Error> {
        let mut saved = BTreeMap::new();
        for line in record
            .expose_secret()
            .lines()
            .filter(|line| !line.is_empty())
        {
            let fields: Vec<&str> = line.split('\t').collect();
            let [host, base_url, token] = fields[..] else {
                return Err(Error::Record);
            };
            let address = normalize_address(base_url)
                .filter(|(normal_host, normal)| normal_host == host && normal == base_url);
            let token = valid(token).filter(|_| token.len() <= TOKEN_LIMIT);
            let (Some(_), Some(token)) = (address, token) else {
                return Err(Error::Record);
            };
            saved.insert(
                host.to_owned(),
                Instance {
                    base_url: base_url.to_owned(),
                    token,
                },
            );
        }
        if saved.len() > MAX_INSTANCES {
            return Err(Error::Record);
        }
        Ok(Self(saved))
    }

    /// The record as the store keeps it.
    fn record(&self) -> SecretString {
        let mut text = Zeroizing::new(String::new());
        for (host, instance) in &self.0 {
            text.push_str(host);
            text.push('\t');
            text.push_str(&instance.base_url);
            text.push('\t');
            text.push_str(instance.token.expose_secret());
            text.push('\n');
        }
        SecretString::from(text.as_str())
    }
}

/// A remote's host as the record keys it: lowercase, without a port.
pub fn hostname(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    match host.rsplit_once(':') {
        Some((name, port))
            if !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit())
                && !name.is_empty() =>
        {
            name.to_owned()
        }
        _ => host,
    }
}

/// The hostname and base address of what the account menu was typed: an
/// `http` or `https` address, `https` when none is given, with an optional
/// port and sub-path, and no user, query, or fragment.
pub fn normalize_address(input: &str) -> Option<(String, String)> {
    let input = input.trim();
    let address = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = url::Url::parse(&address).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host.is_empty() || host.len() > HOST_LIMIT {
        return None;
    }
    let mut base_url = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        base_url.push_str(&format!(":{port}"));
    }
    base_url.push_str(url.path().trim_end_matches('/'));
    (base_url.len() <= ADDRESS_LIMIT).then_some((host, base_url))
}

/// A pasted token without its surrounding whitespace, refused when it is
/// too long or not a run of visible characters.
pub fn checked_token(token: &SecretString) -> Result<SecretString, Error> {
    let token = token.expose_secret().trim();
    if token.len() > TOKEN_LIMIT {
        return Err(Error::TokenLength { limit: TOKEN_LIMIT });
    }
    valid(token).ok_or(Error::Token)
}

/// A token is a run of visible ASCII; anything else would corrupt the header
/// or the record.
fn valid(token: &str) -> Option<SecretString> {
    (!token.is_empty() && token.bytes().all(|byte| byte.is_ascii_graphic())).then(|| token.into())
}

/// Refuses a record the store could keep but never read back, before it
/// replaces one that reads. The bounds keep every record within it; this
/// holds if either changes.
pub fn fits(record: &SecretString) -> Result<(), Error> {
    if record.expose_secret().len() > sheprd_credentials::LIMIT {
        return Err(Error::RecordSize);
    }
    Ok(())
}

/// The folder the GUI config, and so the private credential file, lives in.
fn directory() -> Result<PathBuf, Error> {
    Ok(sheprd_client::local_path::config_root()
        .map_err(Error::Directory)?
        .join("herdr"))
}

/// Every kept instance in `store`.
pub fn read(store: Store) -> Result<Saved, Error> {
    read_in(store, &directory()?)
}

/// [`read`] with the private file in `directory`.
pub fn read_in(store: Store, directory: &Path) -> Result<Saved, Error> {
    match sheprd_credentials::read_entry(store, &ENTRY, directory)? {
        Some(record) => Saved::parse(&record),
        None => Ok(Saved::default()),
    }
}

/// Every kept instance in `store`. A store that cannot be read is logged
/// and kept as empty, so recognizing a remote never fails a sync.
pub fn load(store: Store) -> Saved {
    match directory().and_then(|directory| read_in(store, &directory)) {
        Ok(saved) => saved,
        Err(error) => {
            tracing::warn!(
                category = "orchestrator_gitea",
                %error,
                "Saved Gitea tokens could not be read"
            );
            Saved::default()
        }
    }
}

/// Saves `token` for the instance at `address` in `store`, or forgets the
/// instance's with `None`, keeping the others'. A record not in the
/// expected form is replaced: none of its tokens could be read, and keeping
/// it would refuse every save and forget. A store that cannot be read at
/// all still fails, so a locked keyring never loses tokens.
pub fn save(store: Store, address: &str, token: Option<SecretString>) -> Result<(), Error> {
    save_in(store, &directory()?, address, token)
}

/// [`save`] with the private file in `directory`.
pub fn save_in(
    store: Store,
    directory: &Path,
    address: &str,
    token: Option<SecretString>,
) -> Result<(), Error> {
    let _record = RECORD.lock().unwrap_or_else(PoisonError::into_inner);
    let mut saved = match read_in(store, directory) {
        Err(Error::Record) => {
            tracing::warn!(
                category = "orchestrator_gitea",
                "Replacing saved Gitea tokens not in the expected form"
            );
            Saved::default()
        }
        read => read?,
    };
    saved.set(address, token)?;
    let record = (!saved.0.is_empty()).then(|| saved.record());
    if let Some(record) = &record {
        fits(record)?;
    }
    sheprd_credentials::save_entry(store, &ENTRY, directory, record.as_ref())
}
