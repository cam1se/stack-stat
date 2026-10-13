//! Mode-sensitive input handling; effects are executed outside this module.

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use stack_stat_core::providers::{
    GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider,
};

use super::{App, Mode, NoticeLevel, PointerTarget, Prompt, SortOrder};
use crate::{
    action::{Action, lookup},
    message::{ActionSteps, Effect, Source},
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
        match self.mode {
            Mode::Help => {
                self.mode = Mode::Normal;
                Vec::new()
            }
            Mode::Confirm(_) => self.confirm_key(key),
            Mode::Filter => self.filter_key(key),
            Mode::Normal => self.normal_key(key),
        }
    }

    fn confirm_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('y' | 'Y') => {
                let Mode::Confirm(prompt) = std::mem::replace(&mut self.mode, Mode::Normal) else {
                    unreachable!("confirmation input requires a prompt");
                };
                self.mutation_busy = true;
                self.notify(NoticeLevel::Progress, format!("{}…", prompt.title));
                vec![Effect::Mutate(prompt.steps)]
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.notify(NoticeLevel::Info, "cancelled".into());
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Scroll the hovered pane or select a branch row on mouse click.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, target: PointerTarget) {
        if matches!(self.mode, Mode::Help | Mode::Confirm(_)) {
            return;
        }
        match (mouse.kind, target) {
            (MouseEventKind::ScrollDown, PointerTarget::Detail) => self.scroll_detail(3),
            (MouseEventKind::ScrollUp, PointerTarget::Detail) => self.scroll_detail(-3),
            (MouseEventKind::ScrollDown, PointerTarget::Table | PointerTarget::TableRow(_)) => {
                self.move_selection(1)
            }
            (MouseEventKind::ScrollUp, PointerTarget::Table | PointerTarget::TableRow(_)) => {
                self.move_selection(-1)
            }
            (MouseEventKind::Down(MouseButton::Left), PointerTarget::TableRow(position)) => {
                let visible = self.visible_indices();
                self.choose_visible(&visible, position);
            }
            _ => {}
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
        if key.code == KeyCode::Esc {
            self.dismiss_error();
            return Vec::new();
        }
        let Some(action) = lookup(key) else {
            return Vec::new();
        };
        match action {
            Action::Down => self.move_selection(1),
            Action::Up => self.move_selection(-1),
            Action::Top => self.jump_selection(false),
            Action::Bottom => self.jump_selection(true),
            Action::DetailPageDown => self.scroll_detail(self.detail_page_step()),
            Action::DetailPageUp => self.scroll_detail(-self.detail_page_step()),
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
                return vec![Effect::Mutate(ActionSteps::single(ProviderAction::OpenPr(
                    number,
                )))];
            }
            Action::Checkout | Action::Push | Action::Sync | Action::ToggleDraft => {
                if self.mutation_busy {
                    if !self
                        .status
                        .as_ref()
                        .is_some_and(|notice| notice.level == NoticeLevel::Progress)
                    {
                        self.notify(NoticeLevel::Info, "busy: mutation in progress".into());
                    }
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
                self.mode = Mode::Confirm(Prompt::for_action(operation));
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
        self.notify(NoticeLevel::Info, format!("no PR for {branch}"));
    }

    fn move_selection(&mut self, delta: isize) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        let position = self
            .selected_index
            .saturating_add_signed(delta)
            .min(visible.len() - 1);
        self.choose_visible(&visible, position);
    }

    fn jump_selection(&mut self, last: bool) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        let position = if last { visible.len() - 1 } else { 0 };
        self.choose_visible(&visible, position);
    }

    fn choose_visible(&mut self, visible: &[usize], position: usize) {
        let Some(&index) = visible.get(position) else {
            return;
        };
        let name = self.state.branches[index].name.clone();
        if self.selected_name.as_deref() != Some(name.as_str()) {
            self.detail_scroll = 0;
        }
        self.selected_index = position;
        self.selected_name = Some(name);
        self.follow_worktree = false;
    }
}
