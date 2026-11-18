//! The supervisor's part in lending Sheprd's GitHub sign-in to phones
//! (`control::github`, `docs/ios-github-plan.md`). Sheprd alone renews its
//! GitHub App tokens, as GitHub voids the old pair on every renewal, so a
//! phone gets only the main account's current access token, checked with
//! GitHub just before it goes. Additional accounts and saved hosts' own
//! sign-ins stay here.
//!
//! While a phone is connected, or the daemon says asks wait, the supervisor
//! waits on the daemon for them, one long request at a time, telling it
//! which account Sheprd has. The check with GitHub runs on the background
//! executor, once per take however many asks came.

use super::*;
use crate::SheprdWindow;
use gpui::Context;
use secrecy::SecretString;
use sheprd_remote_control::control::github::{GITHUB_COM, Lease, LeaseAnswer, LeaseAsk, Token};

/// How long after a failed wait the daemon is asked again.
const RETRY: Duration = Duration::from_secs(5);
/// How long the check with GitHub may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Checks a token with GitHub, answering the login it signs in. Blocking.
pub(crate) type Check = fn(&SecretString) -> sheprd_github::Result<String>;

/// Sheprd's main account as a window last saw it.
#[derive(Clone)]
pub(crate) struct Account {
    login: String,
    token: Arc<SecretString>,
}

#[cfg(test)]
impl Account {
    pub(super) fn fixture(login: &str, token: &str) -> Self {
        Self {
            login: login.to_owned(),
            token: Arc::new(token.into()),
        }
    }
}

pub(crate) struct Lending {
    account: Option<Account>,
    check: Check,
    polling: bool,
    retry_at: Option<Instant>,
}

impl Default for Lending {
    fn default() -> Self {
        Self {
            account: None,
            check: check_with_github,
            polling: false,
            retry_at: None,
        }
    }
}

/// The login GitHub says `token` signs in, which proves it still works.
fn check_with_github(token: &SecretString) -> sheprd_github::Result<String> {
    #[derive(serde::Deserialize)]
    struct User {
        login: String,
    }
    // The log's name for the request, never shown.
    let context = "lend sign-in"; // i18n: data
    sheprd_github::rest::<User>(context, token, "/user", CHECK_TIMEOUT, || false)
        .map(|user| user.login)
}

impl SheprdWindow {
    /// Tells the supervisor which main account this window's GitHub menu
    /// holds, for phones to borrow.
    pub(crate) fn publish_github_account(&self, cx: &mut Context<Self>) {
        let account = self.menu.github.profile.as_ref().map(|profile| Account {
            login: profile.login.clone(),
            token: profile.token.clone(),
        });
        let Some(supervisor) = cx.try_global::<Supervisor>() else {
            return;
        };
        let same = match (&supervisor.github.account, &account) {
            (None, None) => true,
            (Some(known), Some(account)) => {
                known.login == account.login && Arc::ptr_eq(&known.token, &account.token)
            }
            _ => false,
        };
        if !same {
            cx.global_mut::<Supervisor>().github.account = account;
        }
    }
}

impl Supervisor {
    /// Waits for phones' asks for the GitHub sign-in, unless that is
    /// underway or no phone could be asking.
    pub(super) fn poll_github_leases(cx: &mut App) {
        let executor = cx.background_executor().clone();
        let supervisor = cx.global_mut::<Self>();
        let asking = matches!(
            &supervisor.health,
            Health::Running(status) if status.connected > 0 || status.github_asks > 0
        );
        let lending = &supervisor.github;
        let waiting = lending.retry_at.is_some_and(|at| Instant::now() < at);
        if !asking || waiting || lending.polling {
            return;
        }
        let Some(address) = supervisor.address.clone() else {
            return;
        };
        supervisor.github.polling = true;
        let send = supervisor.send;
        let login = lending_login(supervisor.github.account.as_ref());
        let task =
            executor.spawn(async move { send(&address, &Request::GitHubLeaseAsks { login }) });
        cx.spawn(async move |cx| {
            let answer = task.await;
            cx.update(|cx| Self::take_lease_asks(cx, answer));
        })
        .detach();
    }

    pub(super) fn take_lease_asks(cx: &mut App, answer: Result<Response, control::Error>) {
        let executor = cx.background_executor().clone();
        let supervisor = cx.global_mut::<Self>();
        supervisor.github.polling = false;
        let Ok(Response::GitHubLeaseAsks { asks }) = answer else {
            // A daemon that does not know the request answers at once.
            supervisor.github.retry_at = Some(Instant::now() + RETRY);
            return;
        };
        supervisor.github.retry_at = None;
        if asks.is_empty() {
            return;
        }
        let Some(address) = supervisor.address.clone() else {
            return;
        };
        let (send, check) = (supervisor.send, supervisor.github.check);
        let account = supervisor.github.account.clone();
        executor
            .spawn(async move {
                for (id, answer) in answers(account.as_ref(), &asks, check) {
                    let request = Request::GitHubLease { id, answer };
                    if let Err(error) = send(&address, &request) {
                        tracing::debug!(%error, "The daemon did not take a GitHub lease");
                    }
                }
            })
            .detach();
    }

    /// Lets `device` borrow this computer's GitHub sign-in, or stops it,
    /// and refreshes the list once the daemon has. `done` gets the outcome
    /// on the UI thread.
    pub(crate) fn share_github(
        cx: &mut App,
        device: DeviceId,
        share: bool,
        done: impl FnOnce(Result<(), control::Error>, &mut App) + 'static,
    ) {
        let Some(client) = Self::client(cx) else {
            return;
        };
        let task = client.request(Request::ShareGitHub { device, share });
        cx.spawn(async move |cx| {
            let result = task.await.map(drop);
            cx.update(|cx| {
                Self::refresh(cx);
                done(result, cx);
            });
        })
        .detach();
    }

    /// The main account phones are told this computer lends, as the
    /// pairing dialog names it.
    pub(crate) fn github_login(cx: &App) -> Option<String> {
        lending_login(cx.try_global::<Self>()?.github.account.as_ref())
    }

    /// Replaces the check with GitHub, as tests do.
    #[cfg(test)]
    pub(crate) fn set_github_for_tests(cx: &mut App, account: Option<(&str, &str)>, check: Check) {
        let lending = &mut cx.global_mut::<Self>().github;
        lending.account = account.map(|(login, token)| Account::fixture(login, token));
        lending.check = check;
    }
}

/// The login the daemon is told, when it is one GitHub could name.
fn lending_login(account: Option<&Account>) -> Option<String> {
    account
        .map(|account| account.login.clone())
        .filter(|login| control::github::valid_login(login))
}

/// The answer to each ask, checking the token with GitHub once for all of
/// them. Blocking when any asks for github.com.
pub(super) fn answers(
    account: Option<&Account>,
    asks: &[LeaseAsk],
    check: Check,
) -> Vec<(control::changes::AskId, LeaseAnswer)> {
    let wanted = asks.iter().any(|ask| ask.host == GITHUB_COM);
    let lease = account
        .filter(|_| wanted)
        .and_then(|account| match check(&account.token) {
            // A token renewed to another account is not the one named.
            Ok(login) if login.eq_ignore_ascii_case(&account.login) => Some(Lease {
                login: account.login.clone(),
                token: Token::new(account.token.as_ref().clone()),
                expires_at: None,
            }),
            Ok(_) => None,
            Err(error) => {
                tracing::debug!(%error, "Sheprd's GitHub sign-in did not check out for a phone");
                None
            }
        })
        .filter(Lease::valid);
    asks.iter()
        .map(|ask| {
            let answer = match &lease {
                Some(lease) if ask.host == GITHUB_COM => LeaseAnswer::Lease(lease.clone()),
                _ => LeaseAnswer::SignedOut,
            };
            (ask.id, answer)
        })
        .collect()
}
