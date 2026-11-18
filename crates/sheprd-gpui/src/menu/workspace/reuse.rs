//! A fresh list turns creation of an already checked-out branch into opening it.
//! Paths come from Git/the daemon, never from parsing a failure message.

use crate::{
    SheprdWindow,
    i18n::{Localize, fl},
    menu::{WorkspaceAction, worktree_open, worktree_source::Pending},
};
use gpui::Context;
use sheprd_client::Method;
use sheprd_remote::{teleport::Host, work};
use std::sync::atomic::AtomicBool;

/// PR checkouts, especially forks, need not have a configured upstream.
pub(super) fn pull_remote(pending: Option<&Pending>) -> Option<(String, String)> {
    let Pending::Item(item) = pending? else {
        return None;
    };
    item.head.as_ref()?;
    let reference = if item.fork_owner.is_some() {
        format!("refs/pull/{}/head", item.number)
    } else {
        format!("refs/heads/{}", item.branch())
    };
    Some((item.remote.name().to_owned(), reference))
}

pub(super) fn pull_notice(result: crate::Result<()>) -> String {
    match result {
        Ok(()) => fl!("worktree-source-reused"),
        Err(error) => fl!("worktree-source-pull-failed", error = error.localize()),
    }
}

impl SheprdWindow {
    pub(super) fn recover_existing_checkout(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(branch) = self
            .menu
            .worktree
            .as_mut()
            .and_then(|source| source.create_branch.take())
        else {
            return false;
        };
        if !self.menu_target_current() || !self.live.status.is_connected() {
            return false;
        }
        let Some(target) = &self.menu.target else {
            return false;
        };
        let Ok(id) = self.endpoints[self.selected_endpoint]
            .connection
            .request_dialog(&target.boot_id, Method::WorktreeList, target.list_params())
        else {
            return false;
        };
        self.menu.creation = Some(id);
        if let Some(source) = &mut self.menu.worktree {
            source.checking = Some((branch, error));
        }
        if let Some(launch) = self.landing_launch.take() {
            let _ = self.take_answer(crate::state::Answer::Landing, launch.request());
        }
        cx.notify();
        true
    }

    pub(super) fn finish_checkout_check(
        &mut self,
        branch: String,
        error: String,
        result: crate::state::DialogResponse,
        cx: &mut Context<Self>,
    ) {
        // A missing match (or a failed listing) keeps the original refusal.
        // Never guess a checkout from Git's diagnostic or a slug collision.
        let existing = result.ok().and_then(|response| {
            let (source, entries) = worktree_open::listing(&response).ok()?;
            entries
                .into_iter()
                .find(|entry| {
                    !entry.is_detached && entry.branch.as_deref() == Some(branch.as_str())
                })
                .map(|entry| (source.repo_key, entry))
        });
        let Some((repository, entry)) = existing else {
            self.menu.creation = None;
            if let Some(source) = &mut self.menu.worktree {
                source.pending = None;
            }
            self.menu.error = Some(error);
            cx.notify();
            return;
        };
        let remote = pull_remote(
            self.menu
                .worktree
                .as_ref()
                .and_then(|source| source.pending.as_ref()),
        );
        let host = self
            .pr_origin()
            .ok_or(sheprd_github::pull_request::Error::UntrustedEndpoint.into())
            .and_then(|_| {
                Host::new(&self.endpoints[self.selected_endpoint].connection.target)
                    .map_err(crate::Error::from)
            });
        let request = format!("reuse-{}", uuid::Uuid::new_v4().simple());
        self.menu.creation = Some(request.clone());
        self.landing_launch = None;
        if let Some(source) = &mut self.menu.worktree {
            source.pending = Some(Pending::Checkout(entry.path.clone()));
            source.reuse_notice = Some(fl!("worktree-source-reusing"));
        }
        let path = entry.path;
        let work_path = path.clone();
        let work = cx.background_executor().spawn(async move {
            let host = host?;
            work::pull_existing(
                &host,
                &repository,
                &work_path,
                &branch,
                remote
                    .as_ref()
                    .map(|(remote, reference)| (remote.as_str(), reference.as_str())),
                &AtomicBool::new(false),
            )
            .map_err(crate::Error::from)
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                if !this.reuse_request_current(&request) {
                    return;
                }
                if let Some(source) = &mut this.menu.worktree {
                    source.reuse_notice = Some(pull_notice(result));
                }
                let opened = (|| {
                    let target = this
                        .menu
                        .target
                        .as_ref()
                        .ok_or(crate::Error::StaleWorkspace)?;
                    let snapshot = this
                        .live
                        .snapshot
                        .as_ref()
                        .ok_or(crate::Error::NoSnapshot)?;
                    let (method, params) =
                        target.request(snapshot, WorkspaceAction::OpenWorktree, &path)?;
                    let boot = target.boot_id.clone();
                    this.request_checkout_direct(&boot, method, params, true)
                })();
                this.queue_checked_checkout(opened, cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn reuse_request_current(&self, request: &str) -> bool {
        self.menu.creation.as_deref() == Some(request)
            && self.menu_target_current()
            && self.live.status.is_connected()
            && self
                .menu
                .target
                .as_ref()
                .zip(self.live.snapshot.as_ref())
                .is_some_and(|(target, snapshot)| target.boot_id == snapshot.boot_id)
    }

    fn queue_checked_checkout(&mut self, result: crate::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(id) => self.menu.creation = Some(id),
            Err(error) => {
                self.menu.creation = None;
                if let Some(source) = &mut self.menu.worktree {
                    source.pending = None;
                    source.reuse_notice = None;
                }
                self.menu.error = Some(error.localize());
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
