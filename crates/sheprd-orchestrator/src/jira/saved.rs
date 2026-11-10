//! Jira Cloud sites and the sign-ins pasted in the account menu: the site
//! address typed, the Atlassian account email, and its API token, one line
//! per site. They are kept together in one credential-store record of
//! `hostname<TAB>base_url<TAB>email<TAB>token` lines, in the desktop keyring,
//! or the private `jira-credentials` file when `[jira] allow_plaintext_credentials`
//! opts in. Every read and write blocks on the store, so callers run on a worker.

use super::Error;
use secrecy::{ExposeSecret, SecretString};
use sheprd_credentials::{Entry, Store};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};
use zeroize::Zeroizing;

/// The most sites one record keeps, which also bounds its size.
pub const MAX_SITES: usize = 32;
/// A DNS name is at most 253 characters.
const HOST_LIMIT: usize = 253;
/// The longest address kept, scheme and sub-path included.
const ADDRESS_LIMIT: usize = 512;
/// An email address is at most 254 characters.
pub const EMAIL_LIMIT: usize = 254;
/// The longest token kept; Atlassian's are a few hundred characters.
pub const TOKEN_LIMIT: usize = 1024;
const _: () = assert!(
    MAX_SITES * (HOST_LIMIT + ADDRESS_LIMIT + EMAIL_LIMIT + TOKEN_LIMIT + 4)
        <= sheprd_credentials::LIMIT
);

const ENTRY: Entry<Error> = Entry {
    service: "dev.herdr.gpui.jira",
    account: "tokens",
    label: "Sheprd Jira API tokens",
    file: c"jira-credentials",
    validate: |record| Saved::parse(record).map(drop),
};

/// Every read-modify-write of the record, so windows saving at once take
/// turns rather than losing each other's sites.
static RECORD: Mutex<()> = Mutex::new(());

/// The sign-in a site takes: an Atlassian account's email and an API token.
#[derive(Clone, Debug)]
pub struct Login {
    pub email: String,
    pub token: SecretString,
}

/// One Jira site: where it is and the sign-in for it.
#[derive(Clone, Debug)]
pub struct Site {
    base_url: String,
    email: String,
    token: SecretString,
}

impl Site {
    /// The address typed, normalized: `https`, host, port, and sub-path,
    /// with no trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn token(&self) -> &SecretString {
        &self.token
    }
}

/// The kept sites, by lowercase hostname without port.
#[derive(Clone, Debug, Default)]
pub struct Saved(BTreeMap<String, Site>);

impl Saved {
    /// The site kept for `host`, whose port is ignored.
    pub fn get(&self, host: &str) -> Option<&Site> {
        self.0.get(&hostname(host))
    }

    /// The sites kept, sorted by hostname.
    pub fn sites(&self) -> impl Iterator<Item = (&str, &Site)> {
        self.0.iter().map(|(host, site)| (host.as_str(), site))
    }

    /// Keeps `login` for the site at `address`, or forgets that site's with
    /// `None`; a bare hostname names it as well.
    pub fn set(&mut self, address: &str, login: Option<Login>) -> Result<(), Error> {
        let (host, base_url) =
            normalize_address(address).ok_or_else(|| Error::Address(address.to_owned()))?;
        match login {
            Some(login) => {
                let email = checked_email(&login.email)?;
                let token = checked_token(&login.token)?;
                if !self.0.contains_key(&host) && self.0.len() >= MAX_SITES {
                    return Err(Error::TooManySites);
                }
                self.0.insert(
                    host,
                    Site {
                        base_url,
                        email,
                        token,
                    },
                );
            }
            None => {
                self.0.remove(&host);
            }
        }
        Ok(())
    }

    /// Reads a record, refusing one that is not lines of a hostname, the
    /// normalized address it belongs to, an email, and a valid token, or that
    /// names more than [`MAX_SITES`].
    pub fn parse(record: &SecretString) -> Result<Self, Error> {
        let mut saved = BTreeMap::new();
        for line in record
            .expose_secret()
            .lines()
            .filter(|line| !line.is_empty())
        {
            let fields: Vec<&str> = line.split('\t').collect();
            let [host, base_url, email, token] = fields[..] else {
                return Err(Error::Record);
            };
            let address = normalize_address(base_url)
                .filter(|(normal_host, normal)| normal_host == host && normal == base_url);
            let email = checked_email(email).ok().filter(|checked| checked == email);
            let token = valid(token).filter(|_| token.len() <= TOKEN_LIMIT);
            let (Some(_), Some(email), Some(token)) = (address, email, token) else {
                return Err(Error::Record);
            };
            saved.insert(
                host.to_owned(),
                Site {
                    base_url: base_url.to_owned(),
                    email,
                    token,
                },
            );
        }
        if saved.len() > MAX_SITES {
            return Err(Error::Record);
        }
        Ok(Self(saved))
    }

    /// The record as the store keeps it.
    fn record(&self) -> SecretString {
        let mut text = Zeroizing::new(String::new());
        for (host, site) in &self.0 {
            for field in [host, &site.base_url, &site.email] {
                text.push_str(field);
                text.push('\t');
            }
            text.push_str(site.token.expose_secret());
            text.push('\n');
        }
        SecretString::from(text.as_str())
    }
}

/// A site's host as the record keys it: lowercase, without a port.
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
/// `https` address, `https` when none is given, with an optional port and
/// sub-path, and no user, query, or fragment. `http` is refused, since the
/// token would cross the network in the clear.
pub fn normalize_address(input: &str) -> Option<(String, String)> {
    let input = input.trim();
    let address = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = url::Url::parse(&address).ok()?;
    if url.scheme() != "https"
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
    let mut base_url = format!("https://{host}");
    if let Some(port) = url.port() {
        base_url.push_str(&format!(":{port}"));
    }
    base_url.push_str(url.path().trim_end_matches('/'));
    (base_url.len() <= ADDRESS_LIMIT).then_some((host, base_url))
}

/// A typed email without its surrounding whitespace, refused when it is too
/// long or not one `@` between two non-empty parts with no whitespace,
/// control character, or `:`, which would split the Basic credential.
pub fn checked_email(email: &str) -> Result<String, Error> {
    let email = email.trim();
    if email.len() > EMAIL_LIMIT {
        return Err(Error::EmailLength { limit: EMAIL_LIMIT });
    }
    let shaped = email.split_once('@').is_some_and(|(name, domain)| {
        !name.is_empty() && !domain.is_empty() && !domain.contains('@')
    });
    if !shaped
        || email
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == ':')
    {
        return Err(Error::Email);
    }
    Ok(email.to_owned())
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

/// Every kept site in `store`.
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

/// Every kept site in `store`. A store that cannot be read is logged and
/// kept as empty, so listing a repository's sources never fails a sync.
pub fn load(store: Store) -> Saved {
    match directory().and_then(|directory| read_in(store, &directory)) {
        Ok(saved) => saved,
        Err(error) => {
            tracing::warn!(
                category = "orchestrator_jira",
                %error,
                "Saved Jira sites could not be read"
            );
            Saved::default()
        }
    }
}

/// Saves `login` for the site at `address` in `store`, or forgets the
/// site's with `None`, keeping the others'. A record not in the expected
/// form is replaced: none of its tokens could be read, and keeping it would
/// refuse every save and forget. A store that cannot be read at all still
/// fails, so a locked keyring never loses tokens.
pub fn save(store: Store, address: &str, login: Option<Login>) -> Result<(), Error> {
    save_in(store, &directory()?, address, login)
}

/// [`save`] with the private file in `directory`.
pub fn save_in(
    store: Store,
    directory: &Path,
    address: &str,
    login: Option<Login>,
) -> Result<(), Error> {
    let _record = RECORD.lock().unwrap_or_else(PoisonError::into_inner);
    let mut saved = match read_in(store, directory) {
        Err(Error::Record) => {
            tracing::warn!(
                category = "orchestrator_jira",
                "Replacing saved Jira sites not in the expected form"
            );
            Saved::default()
        }
        read => read?,
    };
    saved.set(address, login)?;
    let record = (!saved.0.is_empty()).then(|| saved.record());
    if let Some(record) = &record {
        fits(record)?;
    }
    sheprd_credentials::save_entry(store, &ENTRY, directory, record.as_ref())
}
