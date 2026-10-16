//! Selection, visible row ordering, and read-only presentation accessors.

use chrono::{DateTime, Utc};
use stack_stat_core::{
    config::Config,
    model::{CheckHealth, RebaseNeed, StackState},
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use super::{App, Mode, Notice, SortOrder};
use crate::wrap;

impl<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider> App<G, H, S> {
    pub(super) fn sync_selection(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.selected_name = None;
            self.selected_index = 0;
            self.detail_scroll = 0;
            return;
        }
        let selected_position = self.selected_name.as_ref().and_then(|name| {
            visible
                .iter()
                .position(|&index| self.state.branches[index].name == *name)
        });
        let worktree_position = visible
            .iter()
            .position(|&index| self.state.branches[index].is_current);
        let following = self.follow_worktree || self.selected_name.is_none();
        self.selected_index = match (following, worktree_position, selected_position) {
            (true, Some(position), _) | (_, _, Some(position)) => position,
            _ if self.selected_name.is_none() => 0,
            _ => self.selected_index.min(visible.len() - 1),
        };
        // Rows can arrive before Git names the worktree branch; keep following it until then.
        if self.git.is_some() {
            self.follow_worktree = false;
        }
        let name = self.state.branches[visible[self.selected_index]]
            .name
            .clone();
        if self.selected_name.as_deref() != Some(name.as_str()) {
            self.detail_scroll = 0;
        }
        self.selected_name = Some(name);
    }

    /// Branch indexes in filtered and sorted display order.
    #[must_use]
    pub fn visible_indices(&self) -> Vec<usize> {
        let mut indices: Vec<_> = self
            .state
            .branches
            .iter()
            .enumerate()
            .filter_map(|(index, branch)| branch.name.contains(&self.filter).then_some(index))
            .collect();
        match self.sort {
            SortOrder::Stack => {}
            SortOrder::PrNumber => indices
                .sort_by_key(|&index| self.state.branches[index].pr_number.unwrap_or(u32::MAX)),
            SortOrder::Status => indices.sort_by_key(|&index| {
                let worst = self.state.branches[index]
                    .pr
                    .as_ref()
                    .map(|pr| pr.check_summary().worst)
                    .unwrap_or(CheckHealth::None);
                match worst {
                    CheckHealth::Failure => 0,
                    CheckHealth::Running => 1,
                    CheckHealth::Success => 2,
                    CheckHealth::Skipped => 3,
                    CheckHealth::None => 4,
                }
            }),
        }
        indices
    }

    /// The composed stack state.
    #[must_use]
    pub fn state(&self) -> &StackState {
        &self.state
    }
    /// Whether the first GitHub fetch has completed with any outcome.
    #[must_use]
    pub fn has_initial_github_result(&self) -> bool {
        self.github_initial_result
    }
    /// Selected branch, if any is visible.
    #[must_use]
    pub fn selected_branch(&self) -> Option<&str> {
        self.selected_name.as_deref()
    }
    /// Current input context.
    #[must_use]
    pub fn mode(&self) -> &Mode {
        &self.mode
    }
    /// Current footer notice, independent of any confirmation prompt.
    #[must_use]
    pub fn notice(&self) -> Option<&Notice> {
        self.status.as_ref()
    }
    /// Bot reviewer visibility.
    #[must_use]
    pub fn show_bots(&self) -> bool {
        self.show_bots
    }
    /// Initial review density from user configuration.
    #[must_use]
    pub fn reviews_expanded(&self) -> bool {
        self.reviews_expanded
    }
    /// Active branch-name filter.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }
    /// Current sort order.
    #[must_use]
    pub fn sort(&self) -> SortOrder {
        self.sort
    }
    /// Last injected clock value (used only by presentation).
    #[must_use]
    pub fn now(&self) -> DateTime<Utc> {
        self.now
    }
    /// Configuration used to render freshness and poll intervals.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }
    /// Whether any data source is currently fetching.
    #[must_use]
    pub fn is_fetching(&self) -> bool {
        !self.fetching.is_empty()
    }
    /// Notification of a newer stable version, if known.
    #[must_use]
    pub fn update_available(&self) -> Option<&str> {
        self.update_available.as_deref()
    }

    /// Advance presentation time and expire notices using an injected clock.
    pub fn tick(&mut self, now: DateTime<Utc>) {
        self.now = now;
        self.animation.tick();
        if self
            .status
            .as_ref()
            .and_then(|notice| notice.expires_at)
            .is_some_and(|deadline| now >= deadline)
        {
            self.status = None;
            self.notice_source = None;
        }
    }
    /// Presentation-only animation phase and flash lifetime.
    #[must_use]
    pub fn animation(&self) -> &crate::ui::animation::AnimationState {
        &self.animation
    }
    /// Topmost visible detail line; independent of the selected table row.
    #[must_use]
    pub fn detail_scroll(&self) -> usize {
        self.detail_scroll
    }

    /// Visible detail content dimensions provided by the current terminal layout.
    pub fn set_detail_viewport(&mut self, rows: usize, width: usize) {
        self.detail_viewport_rows = rows.max(1);
        self.detail_viewport_width = width.max(1);
        self.detail_scroll = self.detail_scroll.min(self.detail_scroll_max());
    }

    /// Selected position within the filtered/sorted list.
    #[must_use]
    pub fn selected_row_index(&self) -> usize {
        self.selected_index
    }

    pub(super) fn scroll_detail(&mut self, delta: isize) {
        let next = self.detail_scroll.saturating_add_signed(delta);
        self.detail_scroll = next.min(self.detail_scroll_max());
    }

    pub(super) fn detail_page_step(&self) -> isize {
        self.detail_viewport_rows.saturating_sub(2).max(1) as isize
    }

    fn detail_scroll_max(&self) -> usize {
        self.detail_line_count()
            .saturating_sub(self.detail_viewport_rows)
    }

    fn detail_line_count(&self) -> usize {
        let Some(branch) = self.selected_name.as_deref().and_then(|name| {
            self.state
                .branches
                .iter()
                .find(|branch| branch.name == name)
        }) else {
            return 0;
        };
        if branch.pr_number.is_none() && branch.pr.is_none() {
            // Header, branch name, one section gap, and local heading/results.
            return 3
                + wrap::wrap(&branch.name, self.detail_viewport_width).len()
                + usize::from(branch.rebase != RebaseNeed::None)
                + branch.local_checks.len().max(1);
        }
        let titled = branch.pr.as_ref().filter(|pr| !pr.title.is_empty());
        // A titled PR leads with the title, then the branch name indented by two columns.
        let name_and_title = titled.map_or_else(
            || wrap::wrap(&branch.name, self.detail_viewport_width).len(),
            |pr| {
                wrap::wrap(&pr.title, self.detail_viewport_width).len()
                    + wrap::wrap(&branch.name, self.detail_viewport_width.saturating_sub(2)).len()
            },
        );
        let checks = branch
            .pr
            .as_ref()
            .map_or(2, |pr| 1 + pr.checks.len().max(1));
        let reviews = branch.pr.as_ref().map_or(1, |pr| {
            1 + pr
                .reviews
                .iter()
                .filter(|review| self.show_bots || !review.is_bot)
                .count()
        });
        let local = branch.local_checks.len().max(1);
        // Header, wrapped branch name and title, then blank-separated checks, reviews,
        // threads, and local sections.
        1 + name_and_title
            + usize::from(branch.rebase != RebaseNeed::None)
            + checks
            + 1
            + 1
            + reviews
            + 1
            + 1
            + 1
            + 1
            + local
    }
}
