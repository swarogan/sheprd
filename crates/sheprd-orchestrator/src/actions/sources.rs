//! Jira and Gitea changes made from an issue's page, signed in with the
//! sites and instances saved in the account menu.

use super::{Action, Done, Error, Result, Site, perform};
use crate::{gitea, jira};
use std::sync::atomic::{AtomicBool, Ordering};

/// [`perform`] with the Jira sites and Gitea instances saved in the account
/// menu, which a Jira or Gitea change signs in with. Such a change touches
/// no run, so it takes no lease on the repository's actions.
pub fn perform_signed(
    site: &Site,
    action: &Action,
    sites: &jira::saved::Saved,
    instances: &gitea::saved::Saved,
    cancelled: &AtomicBool,
) -> Result<Done> {
    match action {
        Action::Gitea(change) => {
            if cancelled.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            gitea::write::perform(instances.get(&change.source().host), change)?;
        }
        Action::Jira(change) => {
            if cancelled.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            jira::write::perform(sites.get(&change.source().host), change)?;
        }
        _ => return perform(site, action, cancelled),
    }
    Ok(action.done())
}

pub(super) fn jira_done(change: &jira::Change) -> Done {
    match change {
        jira::Change::Transition { .. } => Done::JiraTransitioned,
        jira::Change::Comment { .. } => Done::JiraCommented,
        jira::Change::Assign { .. } => Done::JiraAssigned,
    }
}

pub(super) fn gitea_done(change: &gitea::write::Change) -> Done {
    match change {
        gitea::write::Change::SetState { closed: true, .. } => Done::GiteaClosed,
        gitea::write::Change::SetState { closed: false, .. } => Done::GiteaReopened,
        gitea::write::Change::Comment { .. } => Done::GiteaCommented,
        gitea::write::Change::Assign { .. } => Done::GiteaAssigned,
    }
}
