//! The account menu's Jira page: each Jira Cloud site a sign-in is saved for,
//! whom it signs in as, and a typed address, email, and API token checked with
//! the site before they are saved. Every lookup blocks on the credential store
//! or the network, so it runs on the background executor and lands back here.

// i18n: migrated
use super::Page;
use crate::i18n::fl;
use crate::{SheprdWindow, search_input::SearchInput};
use gpui::{prelude::*, *};
use secrecy::{ExposeSecret, SecretString};
use sheprd_orchestrator::jira::{
    self,
    account::{self, Account},
    saved::{self, Login},
};
use std::sync::Arc;

pub(super) mod keys;
pub(super) mod render;
#[cfg(test)]
mod tests;

/// Where an Atlassian account creates an API token.
const TOKEN_PAGE: &str = "https://id.atlassian.com/manage-profile/security/api-tokens";

/// What is known of a site's sign-in.
pub(crate) enum Lookup {
    /// Being asked, or not yet.
    Unknown,
    /// Whom the site signs the email and token in as.
    Signed(Account),
    Failed(Arc<jira::Error>),
}

/// One site the page lists.
pub(crate) struct SiteStatus {
    /// Its hostname, which names it to Forget.
    pub(crate) host: String,
    /// The address its sign-in is saved under.
    pub(crate) address: String,
    /// The Atlassian email the sign-in was saved with.
    pub(crate) email: String,
    pub(crate) lookup: Lookup,
}

/// The page's state, kept while the menu closes so a reopened page shows
/// the last lookup at once.
#[derive(Default)]
pub(crate) struct JiraMenu {
    pub(crate) sites: Vec<SiteStatus>,
    pub(super) address: Option<Entity<SearchInput>>,
    pub(super) email: Option<Entity<SearchInput>>,
    pub(super) token: Option<Entity<SearchInput>>,
    /// The stop the keyboard selected, which a field with focus outranks.
    pub(crate) selected: Option<keys::Action>,
    /// The page's body, which follows the keyboard's selection.
    pub(super) scroll: ScrollHandle,
    /// A save or forget under way; the buttons wait for it.
    pub(crate) working: bool,
    pub(crate) message: Option<String>,
    /// Why the saved sites could not be read, shown above them.
    pub(crate) problem: Option<String>,
    /// The saved-site read under way, replaced by the next one.
    _task: Option<Task<()>>,
    /// Each site's check, landing on its own row as it finishes.
    _lookups: Vec<Task<()>>,
    /// The save or forget under way, kept apart from lookups so its result
    /// is still shown when the page is opened again.
    _save: Option<Task<()>>,
}

impl JiraMenu {
    /// Stops the lookups under way; their rows keep what they show.
    pub(crate) fn stop_lookups(&mut self) {
        self._task = None;
        self._lookups.clear();
    }
}

/// What the page says in a window that keeps no saved credential.
fn no_store() -> String {
    fl!("jira-menu-no-store")
}

/// What a site's row says.
pub(crate) fn status_line(status: &SiteStatus) -> String {
    match &status.lookup {
        Lookup::Unknown => fl!("jira-menu-checking"),
        Lookup::Signed(account) => fl!(
            "jira-menu-signed-in",
            name = account.name.as_str(),
            email = account.email.as_deref().unwrap_or(status.email.as_str())
        ),
        Lookup::Failed(error) => match **error {
            jira::Error::Authentication => fl!("jira-menu-refused"),
            _ => crate::i18n::errors::Localize::localize(&**error),
        },
    }
}

impl SheprdWindow {
    /// Shows the Jira page: makes its fields, and checks every saved site's
    /// sign-in.
    pub(crate) fn open_jira_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu.page = Some(Page::Jira);
        let field = |placeholder: &'static str, masked: bool, cx: &mut Context<Self>| {
            let input = cx.new(SearchInput::new);
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, cx);
                input.set_masked(masked, cx);
            });
            input
        };
        let jira = &mut self.menu.jira;
        if jira.address.is_none() {
            jira.address = Some(field("example.atlassian.net", false, cx)); // i18n: data
        }
        if jira.email.is_none() {
            jira.email = Some(field("you@example.com", false, cx)); // i18n: data
        }
        if jira.token.is_none() {
            jira.token = Some(field("\u{2022}\u{2022}\u{2022}\u{2022}", true, cx));
        }
        for input in [&jira.address, &jira.email, &jira.token]
            .into_iter()
            .flatten()
        {
            input.update(cx, |input, cx| {
                input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx);
            });
        }
        jira.message = None;
        jira.selected = None;
        if let Some(input) = &jira.token {
            let focus = input.read(cx).focus.clone();
            window.focus(&focus, cx);
        }
        self.check_jira_sites(cx);
        cx.notify();
    }

    /// The store saved sign-ins live in, or `None` in a headless window,
    /// which touches no saved credential.
    fn jira_store(&self) -> Option<sheprd_credentials::Store> {
        self.reads_credentials().then(|| self.config.jira.store())
    }

    /// Lists the saved sites, then checks each sign-in on its own. Without a
    /// store the page lists none.
    pub(crate) fn check_jira_sites(&mut self, cx: &mut Context<Self>) {
        self.menu.jira.stop_lookups();
        let Some(store) = self.jira_store() else {
            self.menu.jira.problem = None;
            self.show_jira_sites(Vec::new(), || Lookup::Unknown);
            return;
        };
        let task = cx.background_spawn(async move { saved::read(store) });
        self.menu.jira._task = Some(cx.spawn(async move |this, cx| {
            let read = task.await;
            let _ = this.update(cx, |this, cx| {
                this.menu.jira.problem = read.as_ref().err().map(|error| {
                    fl!(
                        "jira-menu-record-failed",
                        error = crate::i18n::errors::Localize::localize(error)
                    )
                });
                let kept: Vec<(String, saved::Site)> = read
                    .map(|saved| {
                        saved
                            .sites()
                            .map(|(host, site)| (host.to_owned(), site.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                this.show_jira_sites(
                    kept.iter()
                        .map(|(host, site)| {
                            (
                                host.clone(),
                                site.base_url().to_owned(),
                                site.email().to_owned(),
                            )
                        })
                        .collect(),
                    || Lookup::Unknown,
                );
                this.menu.jira._lookups = kept
                    .into_iter()
                    .map(|(host, site)| this.look_up_jira(host, site, cx))
                    .collect();
                cx.notify();
            });
        }));
    }

    /// Lists `sites` as `(host, address, email)`, each with `lookup`, keeping
    /// a row's last lookup while a new one runs so it holds still.
    pub(crate) fn show_jira_sites(
        &mut self,
        sites: Vec<(String, String, String)>,
        lookup: impl Fn() -> Lookup,
    ) {
        let mut last: Vec<SiteStatus> = std::mem::take(&mut self.menu.jira.sites);
        self.menu.jira.sites = sites
            .into_iter()
            .map(|(host, address, email)| {
                let mut found = lookup();
                if matches!(found, Lookup::Unknown)
                    && let Some(index) = last.iter().position(|status| status.host == host)
                {
                    found = last.swap_remove(index).lookup;
                }
                SiteStatus {
                    host,
                    address,
                    email,
                    lookup: found,
                }
            })
            .collect();
    }

    fn look_up_jira(&self, host: String, site: saved::Site, cx: &mut Context<Self>) -> Task<()> {
        let task = cx.background_spawn(async move {
            account::check(site.base_url(), site.email(), site.token())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(status) = this
                    .menu
                    .jira
                    .sites
                    .iter_mut()
                    .find(|status| status.host == host)
                {
                    status.lookup = match result {
                        Ok(account) => Lookup::Signed(account),
                        Err(error) => Lookup::Failed(Arc::new(error)),
                    };
                }
                cx.notify();
            });
        })
    }

    /// What was typed in a field.
    fn typed_jira(input: &Option<Entity<SearchInput>>, cx: &App) -> String {
        input
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_owned())
            .unwrap_or_default()
    }

    /// Opens the page that creates an API token.
    pub(super) fn open_jira_token_page(&mut self, cx: &mut Context<Self>) {
        cx.open_url(TOKEN_PAGE);
        cx.notify();
    }

    /// Checks the typed address, email, and token with the site, then saves
    /// them.
    pub(super) fn save_jira(&mut self, cx: &mut Context<Self>) {
        if self.menu.jira.working {
            return;
        }
        let address = Self::typed_jira(&self.menu.jira.address, cx);
        let email = Self::typed_jira(&self.menu.jira.email, cx);
        let token = Self::typed_jira(&self.menu.jira.token, cx);
        let localize = |error: &jira::Error| crate::i18n::errors::Localize::localize(error);
        let ready = if address.is_empty() {
            Err(fl!("jira-menu-address-first"))
        } else if saved::normalize_address(&address).is_none() {
            Err(localize(&jira::Error::Address(address.clone())))
        } else if email.is_empty() {
            Err(fl!("jira-menu-email-first"))
        } else if let Err(error) = saved::checked_email(&email) {
            Err(localize(&error))
        } else if token.is_empty() {
            Err(fl!("jira-menu-paste-first"))
        } else {
            self.jira_store().ok_or_else(no_store)
        };
        let store = match ready {
            Ok(store) => store,
            Err(failure) => {
                self.menu.jira.message = Some(failure);
                cx.notify();
                return;
            }
        };
        let token = SecretString::from(token);
        // The field keeps the token until it is saved, so a failed check, such
        // as a mistyped address, does not lose it.
        self.run_jira(cx, Some(token.clone()), move || {
            let checked = account::check(&address, &email, &token)?;
            let login = Login {
                cloud_id: checked.cloud_id.clone(),
                email,
                token,
            };
            saved::save(store, &checked.base_url, Some(login))?;
            Ok(fl!(
                "jira-menu-saved",
                host = checked.host.as_str(),
                user = checked.name.as_str()
            ))
        });
    }

    pub(super) fn forget_jira(&mut self, host: String, cx: &mut Context<Self>) {
        let Some(store) = self.jira_store() else {
            self.menu.jira.message = Some(no_store());
            cx.notify();
            return;
        };
        self.run_jira(cx, None, move || {
            saved::save(store, &host, None)?;
            Ok(fl!("jira-menu-removed", host = host.as_str()))
        });
    }

    /// Runs a save or forget off the UI thread, then says how it went, checks
    /// every site again, and has every window's Orchestrator read the record
    /// again. A successful save empties the token field, unless another token
    /// was typed there meanwhile.
    pub(crate) fn run_jira(
        &mut self,
        cx: &mut Context<Self>,
        saved_token: Option<SecretString>,
        work: impl FnOnce() -> Result<String, jira::Error> + Send + 'static,
    ) {
        self.menu.jira.working = true;
        self.menu.jira.message = None;
        let task = cx.background_spawn(async move { work() });
        self.menu.jira._save = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let changed = result.is_ok();
            let _ = this.update(cx, |this, cx| {
                this.menu.jira.working = false;
                if result.is_ok()
                    && let Some(saved) = &saved_token
                    && let Some(input) = &this.menu.jira.token
                    && input.read(cx).text().trim() == saved.expose_secret()
                {
                    input.update(cx, SearchInput::clear);
                }
                this.menu.jira.message = Some(match result {
                    Ok(done) => done,
                    Err(error) => crate::i18n::errors::Localize::localize(&error),
                });
                cx.notify();
            });
            if changed {
                cx.update(SheprdWindow::refresh_all_orchestrators);
            }
            let _ = this.update(cx, |this, cx| this.check_jira_sites(cx));
        }));
        cx.notify();
    }
}
