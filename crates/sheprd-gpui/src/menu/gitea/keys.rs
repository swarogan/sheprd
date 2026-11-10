//! The Gitea page from the keyboard, as the GitLab page works: Tab and
//! Shift-Tab walk the page's controls and fields in the order they show,
//! Enter or Space uses the selected control, and Escape closes the menu. In a
//! field Enter checks and saves the token typed, and every other key types.

use super::GiteaMenu;
use crate::{SheprdWindow, menu::Page};
use gpui::{prelude::*, *};

/// Which way Tab moves the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Forward,
    Back,
}

/// What a key does on the Gitea page, apart from typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GiteaKey {
    Dismiss,
    /// Enter: saves from a field, else uses the selected control.
    Enter,
    /// Space: uses the selected control; in a field it types a space.
    Space,
    Move(Step),
}

impl GiteaKey {
    fn of(keystroke: &Keystroke) -> Option<Self> {
        let modifiers = keystroke.modifiers;
        let bare = modifiers == Modifiers::default();
        let plain = !modifiers.control && !modifiers.platform && !modifiers.alt;
        Some(match keystroke.key.as_str() {
            "escape" => Self::Dismiss,
            "enter" if bare => Self::Enter,
            "space" if bare => Self::Space,
            "tab" if plain && modifiers.shift => Self::Move(Step::Back),
            "tab" if plain => Self::Move(Step::Forward),
            _ => return None,
        })
    }
}

/// A stop on the Gitea page, as Tab walks them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// The account menu's GitHub tab.
    GitHub,
    /// The account menu's GitLab tab.
    GitLab,
    /// The account menu's Azure DevOps tab.
    AzureDevOps,
    /// Forget the token saved for an instance, by its hostname.
    Forget(String),
    /// The address field, which takes focus rather than being used.
    Address,
    /// The token field, which takes focus rather than being used.
    Token,
    Save,
    Close,
}

impl GiteaMenu {
    /// The page's stops in the order they show, which Tab follows.
    pub(crate) fn actions(&self) -> Vec<Action> {
        let mut actions = vec![Action::GitHub, Action::GitLab, Action::AzureDevOps];
        actions.extend(
            self.instances
                .iter()
                .map(|status| Action::Forget(status.host.clone())),
        );
        actions.extend([Action::Address, Action::Token, Action::Save, Action::Close]);
        actions
    }

    /// The child of the page's scrolling body that holds `action`, in the
    /// order `render_gitea_page` adds them. The tabs, Save, and Close sit
    /// outside the body and always show.
    pub(crate) fn body_child(&self, action: &Action) -> Option<usize> {
        let cards = 1 + usize::from(self.problem.is_some());
        let form = cards + self.instances.len();
        match action {
            Action::Forget(host) => self
                .instances
                .iter()
                .position(|status| status.host == *host)
                .map(|index| cards + index),
            Action::Address => Some(form),
            Action::Token => Some(form + 1),
            Action::GitHub
            | Action::GitLab
            | Action::AzureDevOps
            | Action::Save
            | Action::Close => None,
        }
    }

    /// Whether the keyboard selected `action`.
    pub(crate) fn is_selected(&self, action: &Action) -> bool {
        self.selected.as_ref() == Some(action)
    }
}

impl SheprdWindow {
    /// Uses `action`, as clicking its control does; Forget waits for a save
    /// or forget under way, and a field has nothing to use.
    fn gitea_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::GitHub => self.menu.page = Some(Page::GitHub),
            Action::GitLab => self.open_gitlab_page(window, cx),
            Action::AzureDevOps => self.open_azure_page(window, cx),
            Action::Forget(host) if !self.menu.gitea.working => self.forget_gitea(host, cx),
            Action::Forget(_) | Action::Address | Action::Token => {}
            Action::Save => self.save_gitea(cx),
            Action::Close => self.dismiss_menu(window, cx),
        }
    }

    /// Selects `action`, moving focus into its field or out of the fields,
    /// and scrolls it into view so nothing is used unseen.
    fn select_gitea(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let gitea = &self.menu.gitea;
        let input = match action {
            Action::Address => gitea.address.as_ref(),
            Action::Token => gitea.token.as_ref(),
            _ => None,
        };
        let focus = input.map_or_else(
            || self.menu.focus.clone(),
            |input| input.read(cx).focus.clone(),
        );
        window.focus(&focus, cx);
        if let Some(child) = gitea.body_child(&action) {
            gitea.scroll.scroll_to_item(child);
        }
        self.menu.gitea.selected = Some(action);
    }

    /// Keys while the Gitea page shows.
    pub(in crate::menu) fn gitea_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = GiteaKey::of(&event.keystroke) else {
            return;
        };
        let gitea = &self.menu.gitea;
        let fields = [
            (&gitea.address, Action::Address),
            (&gitea.token, Action::Token),
        ];
        let composing = fields
            .iter()
            .filter_map(|(input, _)| input.as_ref())
            .any(|input| input.read(cx).is_composing());
        // A click can move focus, so the field that has it outranks the
        // selection.
        let field = fields.into_iter().find_map(|(input, action)| {
            let input = input.as_ref()?;
            input.read(cx).focus.is_focused(window).then_some(action)
        });
        if composing || (field.is_some() && key == GiteaKey::Space) {
            return;
        }
        cx.stop_propagation();
        window.prevent_default();
        match key {
            GiteaKey::Dismiss => self.dismiss_menu(window, cx),
            GiteaKey::Move(step) => {
                let actions = gitea.actions();
                let at = field
                    .as_ref()
                    .or(gitea.selected.as_ref())
                    .and_then(|current| actions.iter().position(|action| action == current));
                let next = match (at, step) {
                    (None, Step::Forward) => 0,
                    (None, Step::Back) => actions.len() - 1,
                    (Some(at), Step::Forward) => (at + 1) % actions.len(),
                    (Some(at), Step::Back) => (at + actions.len() - 1) % actions.len(),
                };
                if let Some(action) = actions.get(next).cloned() {
                    self.select_gitea(action, window, cx);
                }
            }
            GiteaKey::Enter if field.is_some() => self.save_gitea(cx),
            GiteaKey::Enter | GiteaKey::Space => {
                // A control that went away since it was selected is not used.
                let selected = gitea
                    .selected
                    .clone()
                    .filter(|selected| gitea.actions().contains(selected));
                match selected {
                    Some(action) => self.gitea_action(action, window, cx),
                    None if key == GiteaKey::Enter => self.save_gitea(cx),
                    None => {}
                }
            }
        }
        cx.notify();
    }
}
