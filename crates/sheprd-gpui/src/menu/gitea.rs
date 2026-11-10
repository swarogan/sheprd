//! The account menu's Gitea page: each instance a token is saved for, whom
//! it signs in as, and a typed address and token checked with the instance
//! before they are saved. Every lookup blocks on the credential store or the
//! network, so it runs on the background executor and lands back here.

// i18n: migrated
use super::Page;
use crate::i18n::fl;
use crate::{SheprdWindow, search_input::SearchInput};
use gpui::{prelude::*, *};
use secrecy::{ExposeSecret, SecretString};
use sheprd_orchestrator::gitea::{
    self,
    account::{self, Account},
    saved,
};
use std::sync::Arc;

pub(super) mod keys;
pub(super) mod render;
#[cfg(test)]
mod tests;

/// What is known of an instance's token.
pub(crate) enum Lookup {
    /// Being asked, or not yet.
    Unknown,
    /// Whom the instance signs the token in as.
    Signed(Account),
    Failed(Arc<gitea::Error>),
}

/// One instance the page lists.
pub(crate) struct InstanceStatus {
    /// Its hostname, which names it to Forget.
    pub(crate) host: String,
    /// The address its token is saved under.
    pub(crate) address: String,
    pub(crate) lookup: Lookup,
}

/// The page's state, kept while the menu closes so a reopened page shows
/// the last lookup at once.
#[derive(Default)]
pub(crate) struct GiteaMenu {
    pub(crate) instances: Vec<InstanceStatus>,
    pub(super) address: Option<Entity<SearchInput>>,
    pub(super) token: Option<Entity<SearchInput>>,
    /// The stop the keyboard selected, which a field with focus outranks.
    pub(crate) selected: Option<keys::Action>,
    /// The page's body, which follows the keyboard's selection.
    pub(super) scroll: ScrollHandle,
    /// A save or forget under way; the buttons wait for it.
    pub(crate) working: bool,
    pub(crate) message: Option<String>,
    /// Why the saved tokens could not be read, shown above the instances.
    pub(crate) problem: Option<String>,
    /// The saved-token read under way, replaced by the next one.
    _task: Option<Task<()>>,
    /// Each instance's check, landing on its own row as it finishes.
    _lookups: Vec<Task<()>>,
    /// The save or forget under way, kept apart from lookups so its result
    /// is still shown when the page is opened again.
    _save: Option<Task<()>>,
}

impl GiteaMenu {
    /// Stops the lookups under way; their rows keep what they show.
    pub(crate) fn stop_lookups(&mut self) {
        self._task = None;
        self._lookups.clear();
    }
}

/// What the page says in a window that keeps no saved credential.
fn no_store() -> String {
    fl!("gitea-menu-no-store")
}

/// What an instance's row says.
pub(crate) fn status_line(status: &InstanceStatus) -> String {
    match &status.lookup {
        Lookup::Unknown => fl!("gitea-menu-checking"),
        Lookup::Signed(account) => fl!("gitea-menu-signed-in", login = account.login.as_str()),
        Lookup::Failed(error) => match **error {
            gitea::Error::Authentication => fl!("gitea-menu-refused"),
            _ => crate::i18n::errors::Localize::localize(&**error),
        },
    }
}

/// Whether `typed` is an `http` address, whose token is sent unencrypted.
pub(crate) fn is_plain_http(typed: &str) -> bool {
    saved::normalize_address(typed).is_some_and(|(_, address)| address.starts_with("http://"))
}

impl SheprdWindow {
    /// Shows the Gitea page: makes its fields, and checks every saved
    /// instance's token.
    pub(crate) fn open_gitea_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu.page = Some(Page::Gitea);
        let field = |placeholder: &'static str, masked: bool, cx: &mut Context<Self>| {
            let input = cx.new(SearchInput::new);
            input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, cx);
                input.set_masked(masked, cx);
            });
            input
        };
        if self.menu.gitea.address.is_none() {
            self.menu.gitea.address = Some(field("https://gitea.example.com", false, cx)); // i18n: data
        }
        if self.menu.gitea.token.is_none() {
            self.menu.gitea.token = Some(field("\u{2022}\u{2022}\u{2022}\u{2022}", true, cx));
        }
        for input in [&self.menu.gitea.address, &self.menu.gitea.token]
            .into_iter()
            .flatten()
        {
            input.update(cx, |input, cx| {
                input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx);
            });
        }
        self.menu.gitea.message = None;
        self.menu.gitea.selected = None;
        if let Some(input) = &self.menu.gitea.token {
            let focus = input.read(cx).focus.clone();
            window.focus(&focus, cx);
        }
        self.check_gitea_instances(cx);
        cx.notify();
    }

    /// The store saved tokens live in, or `None` in a headless window, which
    /// touches no saved credential.
    fn gitea_store(&self) -> Option<sheprd_credentials::Store> {
        self.reads_credentials().then(|| self.config.gitea.store())
    }

    /// Lists the saved instances, then checks each token on its own. Without
    /// a store the page lists none.
    pub(crate) fn check_gitea_instances(&mut self, cx: &mut Context<Self>) {
        self.menu.gitea.stop_lookups();
        let Some(store) = self.gitea_store() else {
            self.menu.gitea.problem = None;
            self.show_gitea_instances(Vec::new(), || Lookup::Unknown);
            return;
        };
        let task = cx.background_spawn(async move { saved::read(store) });
        self.menu.gitea._task = Some(cx.spawn(async move |this, cx| {
            let read = task.await;
            let _ = this.update(cx, |this, cx| {
                this.menu.gitea.problem = read.as_ref().err().map(|error| {
                    fl!(
                        "gitea-menu-record-failed",
                        error = crate::i18n::errors::Localize::localize(error)
                    )
                });
                let kept: Vec<(String, saved::Instance)> = read
                    .map(|saved| {
                        saved
                            .instances()
                            .map(|(host, instance)| (host.to_owned(), instance.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                this.show_gitea_instances(
                    kept.iter()
                        .map(|(host, instance)| (host.clone(), instance.base_url().to_owned()))
                        .collect(),
                    || Lookup::Unknown,
                );
                this.menu.gitea._lookups = kept
                    .into_iter()
                    .map(|(host, instance)| this.look_up_gitea(host, instance, cx))
                    .collect();
                cx.notify();
            });
        }));
    }

    /// Lists `instances` as `(host, address)`, each with `lookup`, keeping a
    /// row's last lookup while a new one runs so it holds still.
    pub(crate) fn show_gitea_instances(
        &mut self,
        instances: Vec<(String, String)>,
        lookup: impl Fn() -> Lookup,
    ) {
        let mut last: Vec<InstanceStatus> = std::mem::take(&mut self.menu.gitea.instances);
        self.menu.gitea.instances = instances
            .into_iter()
            .map(|(host, address)| {
                let mut found = lookup();
                if matches!(found, Lookup::Unknown)
                    && let Some(index) = last.iter().position(|status| status.host == host)
                {
                    found = last.swap_remove(index).lookup;
                }
                InstanceStatus {
                    host,
                    address,
                    lookup: found,
                }
            })
            .collect();
    }

    fn look_up_gitea(
        &self,
        host: String,
        instance: saved::Instance,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let task = cx
            .background_spawn(async move { account::check(instance.base_url(), instance.token()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(status) = this
                    .menu
                    .gitea
                    .instances
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

    /// What was typed in the address field.
    pub(super) fn typed_gitea_address(&self, cx: &App) -> String {
        self.menu
            .gitea
            .address
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_owned())
            .unwrap_or_default()
    }

    /// Checks the typed address and token with the instance, then saves them.
    pub(super) fn save_gitea(&mut self, cx: &mut Context<Self>) {
        if self.menu.gitea.working {
            return;
        }
        let address = self.typed_gitea_address(cx);
        let token = self
            .menu
            .gitea
            .token
            .as_ref()
            .map(|input| input.read(cx).text().trim().to_owned())
            .unwrap_or_default();
        let ready = if address.is_empty() {
            Err(fl!("gitea-menu-address-first"))
        } else if saved::normalize_address(&address).is_none() {
            Err(crate::i18n::errors::Localize::localize(
                &gitea::Error::Address(address.clone()),
            ))
        } else if token.is_empty() {
            Err(fl!("gitea-menu-paste-first"))
        } else {
            self.gitea_store().ok_or_else(no_store)
        };
        let store = match ready {
            Ok(store) => store,
            Err(failure) => {
                self.menu.gitea.message = Some(failure);
                cx.notify();
                return;
            }
        };
        let token = SecretString::from(token);
        // The field keeps the token until it is saved, so a failed check, such
        // as a mistyped address, does not lose it.
        self.run_gitea(cx, Some(token.clone()), move || {
            let checked = account::check(&address, &token)?;
            saved::save(store, &checked.base_url, Some(token))?;
            Ok(fl!(
                "gitea-menu-saved",
                host = checked.host.as_str(),
                user = checked.name.as_str()
            ))
        });
    }

    pub(super) fn forget_gitea(&mut self, host: String, cx: &mut Context<Self>) {
        let Some(store) = self.gitea_store() else {
            self.menu.gitea.message = Some(no_store());
            cx.notify();
            return;
        };
        self.run_gitea(cx, None, move || {
            saved::save(store, &host, None)?;
            Ok(fl!("gitea-menu-removed", host = host.as_str()))
        });
    }

    /// Runs a save or forget off the UI thread, then says how it went, checks
    /// every instance again, and has every window's Orchestrator read the
    /// record again. A successful save empties the token field, unless
    /// another token was typed there meanwhile.
    pub(crate) fn run_gitea(
        &mut self,
        cx: &mut Context<Self>,
        saved_token: Option<SecretString>,
        work: impl FnOnce() -> Result<String, gitea::Error> + Send + 'static,
    ) {
        self.menu.gitea.working = true;
        self.menu.gitea.message = None;
        let task = cx.background_spawn(async move { work() });
        self.menu.gitea._save = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let changed = result.is_ok();
            let _ = this.update(cx, |this, cx| {
                this.menu.gitea.working = false;
                if result.is_ok()
                    && let Some(saved) = &saved_token
                    && let Some(input) = &this.menu.gitea.token
                    && input.read(cx).text().trim() == saved.expose_secret()
                {
                    input.update(cx, SearchInput::clear);
                }
                this.menu.gitea.message = Some(match result {
                    Ok(done) => done,
                    Err(error) => crate::i18n::errors::Localize::localize(&error),
                });
                cx.notify();
            });
            if changed {
                cx.update(SheprdWindow::refresh_all_orchestrators);
            }
            let _ = this.update(cx, |this, cx| this.check_gitea_instances(cx));
        }));
        cx.notify();
    }
}
