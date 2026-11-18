//! The state a test puts into a `CoreTerminals` that no core answers, so a
//! tab or a checkout reads as one the hub had already filled.

use super::{CoreTerminals, Host, Panes, Scope, TabId, TabPanes, TerminalInfo};
// Only the cloud-gated reader of `wanted` names these.
#[cfg(feature = "cloud")]
use super::{Purpose, Spawn};

impl CoreTerminals {
    /// Shows `panes` as `tab`, a mirror when `observe`, with what herdr-core
    /// reports of `infos`, as a restored tab and a finished list would.
    pub(crate) fn show_for_test(
        &mut self,
        tab: TabId,
        panes: Panes,
        observe: bool,
        infos: Vec<TerminalInfo>,
    ) {
        let mut shown = TabPanes::new(Host::Local, panes, observe);
        for info in infos {
            self.infos.insert(info);
        }
        shown.keep_root(&self.infos);
        self.tabs.insert(tab, shown);
    }

    /// Takes `identity` as what Git answered for the folder `cwd`.
    pub(crate) fn seed_checkout_for_test(
        &mut self,
        cwd: &str,
        identity: Option<sheprd_vcs::git::Identity>,
    ) {
        self.checkouts.seed(cwd, identity);
    }

    /// The latest terminal waiting to start, as no core answers in tests.
    #[cfg(feature = "cloud")]
    pub(crate) fn last_wanted_for_test(&self) -> Option<(&Purpose, &Spawn)> {
        let (purpose, _, spawn, _) = self.wanted.back()?;
        Some((purpose, spawn))
    }

    /// Places `tab` in `workspace` of `scope`, as showing the workspace does.
    pub(crate) fn place_for_test(&mut self, tab: TabId, scope: Scope, workspace: &str) {
        if let Some(shown) = self.tabs.get_mut(&tab) {
            shown.workspace = Some((scope, workspace.to_owned()));
        }
    }
}
