//! The Jira page from the keyboard, as the GitLab page works: Tab and
//! Shift-Tab walk the page's controls and fields in the order they show,
//! Enter or Space uses the selected control, and Escape closes the menu. In a
//! field Enter checks and saves the sign-in typed, and every other key types.

use super::JiraMenu;
use crate::{SheprdWindow, menu::Page};
use gpui::{prelude::*, *};

/// Which way Tab moves the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Forward,
    Back,
}

/// What a key does on the Jira page, apart from typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JiraKey {
    Dismiss,
    /// Enter: saves from a field, else uses the selected control.
    Enter,
    /// Space: uses the selected control; in a field it types a space.
    Space,
    Move(Step),
}

impl JiraKey {
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

/// A stop on the Jira page, as Tab walks them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// The account menu's GitHub tab.
    GitHub,
    /// The account menu's GitLab tab.
    GitLab,
    /// The account menu's Gitea tab.
    Gitea,
    /// The account menu's Azure DevOps tab.
    AzureDevOps,
    /// Forget the sign-in saved for a site, by its hostname.
    Forget(String),
    /// The address field, which takes focus rather than being used.
    Address,
    /// The email field, which takes focus rather than being used.
    Email,
    /// The token field, which takes focus rather than being used.
    Token,
    /// The link to where an API token is created.
    CreateToken,
    Save,
    Close,
}

impl JiraMenu {
    /// The page's stops in the order they show, which Tab follows.
    pub(crate) fn actions(&self) -> Vec<Action> {
        let mut actions = vec![
            Action::GitHub,
            Action::GitLab,
            Action::Gitea,
            Action::AzureDevOps,
        ];
        actions.extend(
            self.sites
                .iter()
                .map(|status| Action::Forget(status.host.clone())),
        );
        actions.extend([
            Action::Address,
            Action::Email,
            Action::Token,
            Action::CreateToken,
            Action::Save,
            Action::Close,
        ]);
        actions
    }

    /// The child of the page's scrolling body that holds `action`, in the
    /// order `render_jira_page` adds them. The tabs, Save, and Close sit
    /// outside the body and always show.
    pub(crate) fn body_child(&self, action: &Action) -> Option<usize> {
        let cards = 1 + usize::from(self.problem.is_some());
        let form = cards + self.sites.len();
        match action {
            Action::Forget(host) => self
                .sites
                .iter()
                .position(|status| status.host == *host)
                .map(|index| cards + index),
            Action::Address => Some(form),
            Action::Email => Some(form + 1),
            Action::Token => Some(form + 2),
            Action::CreateToken => Some(form + 3),
            Action::GitHub
            | Action::GitLab
            | Action::Gitea
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
    fn jira_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::GitHub => self.menu.page = Some(Page::GitHub),
            Action::GitLab => self.open_gitlab_page(window, cx),
            Action::Gitea => self.open_gitea_page(window, cx),
            Action::AzureDevOps => self.open_azure_page(window, cx),
            Action::Forget(host) if !self.menu.jira.working => self.forget_jira(host, cx),
            Action::Forget(_) | Action::Address | Action::Email | Action::Token => {}
            Action::CreateToken => self.open_jira_token_page(cx),
            Action::Save => self.save_jira(cx),
            Action::Close => self.dismiss_menu(window, cx),
        }
    }

    /// Selects `action`, moving focus into its field or out of the fields,
    /// and scrolls it into view so nothing is used unseen.
    fn select_jira(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let jira = &self.menu.jira;
        let input = match action {
            Action::Address => jira.address.as_ref(),
            Action::Email => jira.email.as_ref(),
            Action::Token => jira.token.as_ref(),
            _ => None,
        };
        let focus = input.map_or_else(
            || self.menu.focus.clone(),
            |input| input.read(cx).focus.clone(),
        );
        window.focus(&focus, cx);
        if let Some(child) = jira.body_child(&action) {
            jira.scroll.scroll_to_item(child);
        }
        self.menu.jira.selected = Some(action);
    }

    /// Keys while the Jira page shows.
    pub(in crate::menu) fn jira_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = JiraKey::of(&event.keystroke) else {
            return;
        };
        let jira = &self.menu.jira;
        let fields = [
            (&jira.address, Action::Address),
            (&jira.email, Action::Email),
            (&jira.token, Action::Token),
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
        if composing || (field.is_some() && key == JiraKey::Space) {
            return;
        }
        cx.stop_propagation();
        window.prevent_default();
        match key {
            JiraKey::Dismiss => self.dismiss_menu(window, cx),
            JiraKey::Move(step) => {
                let actions = jira.actions();
                let at = field
                    .as_ref()
                    .or(jira.selected.as_ref())
                    .and_then(|current| actions.iter().position(|action| action == current));
                let next = match (at, step) {
                    (None, Step::Forward) => 0,
                    (None, Step::Back) => actions.len() - 1,
                    (Some(at), Step::Forward) => (at + 1) % actions.len(),
                    (Some(at), Step::Back) => (at + actions.len() - 1) % actions.len(),
                };
                if let Some(action) = actions.get(next).cloned() {
                    self.select_jira(action, window, cx);
                }
            }
            JiraKey::Enter if field.is_some() => self.save_jira(cx),
            JiraKey::Enter | JiraKey::Space => {
                // A control that went away since it was selected is not used.
                let selected = jira
                    .selected
                    .clone()
                    .filter(|selected| jira.actions().contains(selected));
                match selected {
                    Some(action) => self.jira_action(action, window, cx),
                    None if key == JiraKey::Enter => self.save_jira(cx),
                    None => {}
                }
            }
        }
        cx.notify();
    }
}
