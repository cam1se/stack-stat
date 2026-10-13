//! Mode-sensitive input handling; effects are executed outside this module.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use stack_stat_core::providers::{
    GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider,
};

use super::{App, Mode, SortOrder, StatusLine};
use crate::{
    action::{Action, lookup},
    message::{Effect, Source},
};

impl<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider> App<G, H, S> {
    /// Handle one terminal key without performing I/O; return requested effects.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Vec::new();
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return vec![Effect::Quit];
        }
        match self.mode.clone() {
            Mode::Help => {
                self.mode = Mode::Normal;
                Vec::new()
            }
            Mode::Confirm(action) => {
                self.mode = Mode::Normal;
                if key.code == KeyCode::Char('y') {
                    self.mutation_busy = true;
                    vec![Effect::Mutate(action)]
                } else {
                    self.status = StatusLine {
                        text: "cancelled".into(),
                        is_error: false,
                    };
                    Vec::new()
                }
            }
            Mode::Filter => self.filter_key(key),
            Mode::Normal => self.normal_key(key),
        }
    }

    fn filter_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => {
                self.filter.clear();
                self.mode = Mode::Normal;
                self.sync_selection();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.sync_selection();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.push(ch);
                self.sync_selection();
            }
            _ => {}
        }
        Vec::new()
    }

    fn normal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let Some(action) = lookup(key) else {
            return Vec::new();
        };
        match action {
            Action::Down => self.move_selection(1),
            Action::Up => self.move_selection(-1),
            Action::Top => self.jump_selection(false),
            Action::Bottom => self.jump_selection(true),
            Action::ToggleDetail => self.detail_open = !self.detail_open,
            Action::Filter => self.mode = Mode::Filter,
            Action::Sort => {
                self.sort = match self.sort {
                    SortOrder::Stack => SortOrder::PrNumber,
                    SortOrder::PrNumber => SortOrder::Status,
                    SortOrder::Status => SortOrder::Stack,
                };
                self.sync_selection();
            }
            Action::ToggleBots => self.show_bots = !self.show_bots,
            Action::Help => self.mode = Mode::Help,
            Action::Quit => return vec![Effect::Quit],
            Action::Refresh => return vec![Effect::Refresh(Source::All)],
            Action::OpenPr => {
                let Some(number) = self.selected().and_then(|branch| branch.pr_number) else {
                    self.no_pr();
                    return Vec::new();
                };
                return vec![Effect::Mutate(ProviderAction::OpenPr(number))];
            }
            Action::Checkout | Action::Push | Action::Sync | Action::ToggleDraft => {
                if self.mutation_busy {
                    self.status = StatusLine {
                        text: "busy: mutation in progress".into(),
                        is_error: false,
                    };
                    return Vec::new();
                }
                let operation = match action {
                    Action::Checkout => self
                        .selected()
                        .map(|branch| ProviderAction::Checkout(branch.name.clone())),
                    Action::Push => Some(ProviderAction::PushStack),
                    Action::Sync => Some(ProviderAction::SyncStack),
                    Action::ToggleDraft => self.selected().and_then(|branch| {
                        branch.pr.as_ref().map(|pr| ProviderAction::ToggleDraft {
                            number: pr.number,
                            make_ready: pr.is_draft,
                        })
                    }),
                    _ => unreachable!(),
                };
                let Some(operation) = operation else {
                    self.no_pr();
                    return Vec::new();
                };
                self.status = StatusLine {
                    text: format!("{}? y/n", prompt(&operation)),
                    is_error: false,
                };
                self.mode = Mode::Confirm(operation);
            }
        }
        Vec::new()
    }

    fn selected(&self) -> Option<&stack_stat_core::model::BranchState> {
        let name = self.selected_name.as_deref()?;
        self.state
            .branches
            .iter()
            .find(|branch| branch.name == name)
    }

    fn no_pr(&mut self) {
        let branch = self.selected_name.as_deref().unwrap_or("selected branch");
        self.status = StatusLine {
            text: format!("no PR for {branch}"),
            is_error: false,
        };
    }

    fn move_selection(&mut self, direction: i8) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        self.selected_index = if direction > 0 {
            (self.selected_index + 1).min(visible.len() - 1)
        } else {
            self.selected_index.saturating_sub(1)
        };
        self.selected_name = Some(
            self.state.branches[visible[self.selected_index]]
                .name
                .clone(),
        );
    }

    fn jump_selection(&mut self, last: bool) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        self.selected_index = if last { visible.len() - 1 } else { 0 };
        self.selected_name = Some(
            self.state.branches[visible[self.selected_index]]
                .name
                .clone(),
        );
    }
}

fn prompt(action: &ProviderAction) -> String {
    match action {
        ProviderAction::OpenPr(number) => format!("Open PR #{number}"),
        ProviderAction::Checkout(branch) => format!("Checkout {branch}"),
        ProviderAction::PushStack => "Push stack".into(),
        ProviderAction::SyncStack => "Sync stack".into(),
        ProviderAction::ToggleDraft { number, make_ready } => format!(
            "Mark PR #{number} {}",
            if *make_ready { "ready" } else { "draft" }
        ),
    }
}
