//! A phone's rename or close of a workspace this app keeps for Herdr 2 on
//! this computer, or deletion of its linked worktree, made as the
//! workspace's menu makes them. A deletion is never forced: a checkout with
//! changes stays, and the phone is told why.

use crate::{SheprdWindow, browser::Scope, remote::supervisor::Supervisor};
use gpui::{Context, Window};
use sheprd_remote::work;
use sheprd_remote_control::control::{
    changes::AskId,
    layout::{Answer, Failure},
};
use std::sync::atomic::AtomicBool;

/// What a phone hears when a deletion fails.
fn removal_failure(error: &crate::Error) -> Failure {
    match error {
        crate::Error::Remote(sheprd_remote::Error::Work(work::Error::Dirty)) => Failure::Dirty,
        crate::Error::StaleWorkspace => Failure::Gone,
        _ => Failure::Refused,
    }
}

impl SheprdWindow {
    /// The endpoint and scope of this computer's host when this window
    /// keeps `workspace` there, whichever host it shows.
    fn phone_kept(&self, workspace: &str, cx: &Context<Self>) -> Option<(usize, Scope)> {
        let index = self.core_host()?;
        let scope = crate::browser::scope(&self.endpoints[index]);
        cx.try_global::<crate::browser::Store>()
            .is_some_and(|store| store.kept_workspace(&scope, workspace).is_some())
            .then_some((index, scope))
    }

    /// Renames a workspace this window keeps for Herdr 2 on this computer;
    /// false when it keeps none by that id, so another window may.
    pub(super) fn phone_rename_workspace(
        &mut self,
        workspace: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, scope)) = self.phone_kept(workspace, cx) else {
            return false;
        };
        crate::browser::Store::update(cx, |store| store.rename_kept(&scope, workspace, name));
        self.refresh_core_session(cx);
        cx.notify();
        true
    }

    /// Closes a kept workspace as its menu's Close does: its tabs close and
    /// their terminals end. False when this window keeps none by that id.
    pub(super) fn phone_close_workspace(
        &mut self,
        workspace: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, scope)) = self.phone_kept(workspace, cx) else {
            return false;
        };
        self.close_kept_workspace_in(&scope, workspace, window, cx);
        true
    }

    /// Deletes a kept workspace's linked checkout off the UI thread, then
    /// closes the workspace, as its menu's Delete does without force. The
    /// phone's answer follows the removal; `None` when this window keeps no
    /// workspace by that id.
    pub(super) fn phone_remove_worktree(
        &mut self,
        id: AskId,
        workspace: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Option<Answer>> {
        let (index, scope) = self.phone_kept(workspace, cx)?;
        let identity = self.kept_checkout_identity(index, workspace);
        let path = self
            .sessions(index)
            .core
            .as_ref()
            .and_then(|session| session.workspace(workspace))
            .map(|kept| kept.cwd.clone())
            .filter(|cwd| !cwd.is_empty());
        let (Some(identity), Some(path)) = (identity, path) else {
            return Some(Some(Answer::Failed(Failure::NotWorktree)));
        };
        let target = self.endpoints[index].connection.target.clone();
        let removal = cx.background_executor().spawn(async move {
            let cancelled = AtomicBool::new(false);
            crate::menu::remove_kept_checkout_on(
                &target,
                Some(identity),
                &path,
                false,
                None,
                &cancelled,
            )
        });
        let workspace = workspace.to_owned();
        cx.spawn_in(window, async move |this, cx| {
            let result = removal.await;
            let answer = match &result {
                Ok(()) => Answer::Done,
                Err(error) => {
                    tracing::debug!(%error, "A phone's worktree deletion was refused");
                    Answer::Failed(removal_failure(error))
                }
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if result.is_ok() {
                    this.close_kept_workspace_in(&scope, &workspace, window, cx);
                }
                Supervisor::send_layout_answer(cx, id, answer);
            });
        })
        .detach();
        Some(None)
    }
}

#[cfg(test)]
mod tests;
