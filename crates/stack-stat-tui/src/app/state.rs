//! Selection, visible row ordering, and read-only presentation accessors.

use chrono::{DateTime, Utc};
use stack_stat_core::{
    config::Config,
    model::{CheckHealth, StackState},
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use super::{App, Mode, SortOrder, StatusLine};

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
        self.selected_index = selected_position.unwrap_or_else(|| {
            if self.selected_name.is_none() {
                visible
                    .iter()
                    .position(|&index| self.state.branches[index].is_current)
                    .unwrap_or(0)
            } else {
                self.selected_index.min(visible.len() - 1)
            }
        });
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
    /// Selected branch, if any is visible.
    #[must_use]
    pub fn selected_branch(&self) -> Option<&str> {
        self.selected_name.as_deref()
    }
    /// Current input context.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode.clone()
    }
    /// Footer feedback.
    #[must_use]
    pub fn status_line(&self) -> &StatusLine {
        &self.status
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

    /// Advance presentation time with an injected clock.
    pub fn tick(&mut self, now: DateTime<Utc>) {
        self.now = now;
        self.animation.tick();
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

    /// Visible detail content rows provided by the current terminal layout.
    pub fn set_detail_viewport_rows(&mut self, rows: usize) {
        self.detail_viewport_rows = rows.max(1);
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
        let title = branch.pr.as_ref().is_some_and(|pr| !pr.title.is_empty());
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
        // header, optional title and rebase warning, then blank-separated checks, reviews, local sections
        1 + usize::from(title)
            + usize::from(branch.needs_rebase)
            + 1
            + checks
            + 1
            + 1
            + reviews
            + 1
            + 1
            + local
    }
}
