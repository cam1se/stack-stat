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
            return;
        }
        let selected_position = self.selected_name.as_ref().and_then(|name| {
            visible
                .iter()
                .position(|&index| self.state.branches[index].name == *name)
        });
        self.selected_index =
            selected_position.unwrap_or_else(|| self.selected_index.min(visible.len() - 1));
        self.selected_name = Some(
            self.state.branches[visible[self.selected_index]]
                .name
                .clone(),
        );
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
    /// Whether the selected row's details are visible.
    #[must_use]
    pub fn detail_open(&self) -> bool {
        self.detail_open
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
    }
}
