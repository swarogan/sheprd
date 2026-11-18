//! Where a phone's ask for Sheprd's GitHub sign-in waits for Sheprd, which
//! alone holds and renews it: Sheprd takes what is queued here with
//! `Request::GitHubLeaseAsks` and answers each by id. Bounded, and an ask
//! whose phone gave up is forgotten. Each take also says which account
//! Sheprd has now, which a phone's home lists.

use super::Error;
use crate::control::{
    changes::AskId,
    github::{LeaseAnswer, LeaseAsk, valid_login},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::Duration,
};
use tokio::sync::{oneshot, watch};

/// Asks queued or being answered at once, from every phone.
const MAX_WAITING: usize = 8;

pub(crate) struct Asks {
    arrived: Condvar,
    inner: Mutex<Inner>,
    /// Sheprd's main github.com account as it last said.
    login: watch::Sender<Option<String>>,
}

impl Default for Asks {
    fn default() -> Self {
        Self {
            arrived: Condvar::new(),
            inner: Mutex::default(),
            login: watch::Sender::new(None),
        }
    }
}

#[derive(Default)]
struct Inner {
    next: u64,
    queued: VecDeque<LeaseAsk>,
    waiting: HashMap<AskId, oneshot::Sender<LeaseAnswer>>,
}

/// An ask on its way. Dropped, it is withdrawn.
pub(crate) struct Pending<'a> {
    answer: oneshot::Receiver<LeaseAnswer>,
    asks: &'a Asks,
    id: AskId,
}

impl Pending<'_> {
    /// Sheprd's answer; `None` if the ask was dropped.
    pub(crate) async fn answer(&mut self) -> Option<LeaseAnswer> {
        (&mut self.answer).await.ok()
    }
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        let mut inner = self.asks.inner();
        inner.waiting.remove(&self.id);
        inner.queued.retain(|ask| ask.id != self.id);
    }
}

impl Asks {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sheprd's main github.com account, as it last said.
    pub(crate) fn login(&self) -> Option<String> {
        self.login.borrow().clone()
    }

    /// Changes whenever Sheprd's account does.
    pub(crate) fn watch_login(&self) -> watch::Receiver<Option<String>> {
        self.login.subscribe()
    }

    /// How many asks wait for Sheprd to take them.
    pub(crate) fn queued(&self) -> usize {
        self.inner().queued.len()
    }

    /// Queues an ask for the sign-in Sheprd holds for `host`.
    pub(crate) fn ask(&self, host: String) -> Result<Pending<'_>, Error> {
        let mut inner = self.inner();
        if inner.waiting.len() >= MAX_WAITING {
            return Err(Error::Busy);
        }
        let id = AskId(inner.next);
        inner.next += 1;
        let (sender, answer) = oneshot::channel();
        inner.waiting.insert(id, sender);
        inner.queued.push_back(LeaseAsk { id, host });
        self.arrived.notify_all();
        Ok(Pending {
            answer,
            asks: self,
            id,
        })
    }

    /// Takes every queued ask, waiting up to `wait` for one, and keeps
    /// `login` as Sheprd's account. Blocking: runs on the control
    /// connection's own thread.
    pub(crate) fn take(&self, login: Option<String>, wait: Duration) -> Vec<LeaseAsk> {
        let login = login.filter(|login| valid_login(login));
        self.login.send_if_modified(|current| {
            let changed = *current != login;
            *current = login;
            changed
        });
        let (mut inner, _) = self
            .arrived
            .wait_timeout_while(self.inner(), wait, |inner| inner.queued.is_empty())
            .unwrap_or_else(PoisonError::into_inner);
        inner.queued.drain(..).collect()
    }

    /// Hands Sheprd's answer to the phone waiting for it, if one still is.
    /// A lease past the bounds never reaches it.
    pub(crate) fn answer(&self, id: AskId, answer: LeaseAnswer) {
        let answer = match answer {
            LeaseAnswer::Lease(lease) if !lease.valid() => {
                tracing::warn!("Sheprd's GitHub lease was out of bounds");
                LeaseAnswer::SignedOut
            }
            answer => answer,
        };
        if let Some(waiting) = self.inner().waiting.remove(&id) {
            let _ = waiting.send(answer);
        }
    }
}
