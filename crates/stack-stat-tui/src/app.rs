//! Application state: successful snapshots, selection, modes, and feedback.

use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Utc};
use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StoreSnapshot, compose},
    config::{Config, ReviewsDefault},
    model::{StackState, StackStatus},
    providers::{GitHubError, GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{
    message::{AppMessage, Source},
    ui::animation::{AnimationState, diff_cells},
};

mod effects;
mod feedback;
mod keys;
mod notice;
mod prompt;
mod state;
pub use effects::execute;
pub use notice::{Notice, NoticeLevel};
pub use prompt::Prompt;

/// Current input context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Branch table and global commands.
    Normal,
    /// Incremental branch-name search.
    Filter,
    /// Awaiting an explicit y/n response for a mutation.
    Confirm(Prompt),
    /// Help overlay; next key closes it.
    Help,
}

/// UI location identified by the renderer for a mouse event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerTarget {
    /// A branch row at its visible-list index.
    TableRow(usize),
    /// Table border, header, or empty table space.
    Table,
    /// Selected branch's detail pane.
    Detail,
    /// Neither scrollable region.
    Outside,
}

/// Row sort order, cyclically selected with `s`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    /// GitHub-provided trunk-to-top order.
    #[default]
    Stack,
    /// Numeric PR number, ascending.
    PrNumber,
    /// Worst check status first, then stack order.
    Status,
}

/// Generic, directly drivable TUI state with injectable I/O providers.
pub struct App<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider> {
    git_provider: Arc<G>,
    github_provider: Arc<H>,
    store_provider: Arc<S>,
    config: Config,
    git: Option<GitSnapshot>,
    github: Option<GitHubSnapshot>,
    github_initial_result: bool,
    store: Option<StoreSnapshot>,
    state: StackState,
    selected_name: Option<String>,
    selected_index: usize,
    /// Automatic selection tracks the worktree branch until Git reports it or the user picks a row.
    follow_worktree: bool,
    detail_scroll: usize,
    detail_viewport_rows: usize,
    detail_viewport_width: usize,
    mode: Mode,
    sort: SortOrder,
    filter: String,
    show_bots: bool,
    reviews_expanded: bool,
    status: Option<Notice>,
    notice_source: Option<Source>,
    mutation_busy: bool,
    fetching: HashSet<Source>,
    now: DateTime<Utc>,
    update_available: Option<String>,
    animation: AnimationState,
}

impl<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider> App<G, H, S> {
    /// Create an empty, loading dashboard with the specified providers and settings.
    #[must_use]
    pub fn new(
        git_provider: Arc<G>,
        github_provider: Arc<H>,
        store_provider: Arc<S>,
        config: Config,
    ) -> Self {
        let show_bots = config.reviews.show_bots;
        let reviews_expanded = config.display.reviews_default == ReviewsDefault::Expanded;
        let now = Utc::now();
        let status = config
            .warnings
            .first()
            .cloned()
            .map(|text| Notice::new(NoticeLevel::Warn, text, now));
        Self {
            git_provider,
            github_provider,
            store_provider,
            config,
            git: None,
            github: None,
            github_initial_result: false,
            store: None,
            state: StackState::default(),
            selected_name: None,
            selected_index: 0,
            follow_worktree: true,
            detail_scroll: 0,
            detail_viewport_rows: 12,
            detail_viewport_width: 80,
            mode: Mode::Normal,
            sort: SortOrder::Stack,
            filter: String::new(),
            show_bots,
            reviews_expanded,
            status,
            notice_source: None,
            mutation_busy: false,
            fetching: HashSet::new(),
            now,
            update_available: None,
            animation: AnimationState::default(),
        }
    }

    /// Apply a source update without losing previous successful data on errors.
    pub fn apply(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::Git(result) => {
                self.fetching.remove(&Source::Git);
                match result {
                    Ok(snapshot) => {
                        self.git = Some(snapshot);
                        self.clear_error(Source::Git);
                    }
                    Err(error) => self.error(Source::Git, format!("! git fetch failed: {error}")),
                }
            }
            AppMessage::GitHub(result) => {
                self.github_initial_result = true;
                self.fetching.remove(&Source::GitHub);
                match result {
                    Ok(snapshot) => {
                        self.github = Some(snapshot);
                        self.clear_error(Source::GitHub);
                    }
                    Err(GitHubError::NotInStack) => {
                        let repo = self
                            .github
                            .as_ref()
                            .map(|snapshot| snapshot.repo.clone())
                            .unwrap_or_default();
                        self.github = Some(GitHubSnapshot {
                            stack: stack_stat_core::compose::StackOutcome::NotInStack,
                            details: Default::default(),
                            repo,
                            // A bare error does not say which branch it was for.
                            branch: String::new(),
                            fetched_at: self.now,
                        });
                        self.clear_error(Source::GitHub);
                    }
                    Err(GitHubError::RateLimited { reset_hint }) => self.error(
                        Source::GitHub,
                        format!(
                            "! rate limited, resumes in {}",
                            reset_hint.unwrap_or_else(|| "5m".into())
                        ),
                    ),
                    Err(error) => self.error(Source::GitHub, format!("! fetch failed: {error}")),
                }
            }
            AppMessage::Store(result) => {
                self.fetching.remove(&Source::Store);
                match result {
                    Ok(snapshot) => {
                        self.clear_error(Source::Store);
                        if let Some(warning) = snapshot.warnings.first() {
                            self.notify(NoticeLevel::Warn, format!("! {warning}"));
                        }
                        self.store = Some(snapshot);
                    }
                    Err(error) => self.error(Source::Store, format!("! local checks: {error}")),
                }
            }
            AppMessage::FetchStarted(source) => {
                self.fetching.insert(source);
            }
            AppMessage::MutationDone { steps, result } => {
                self.complete_mutation(steps, result);
            }
            AppMessage::UpdateAvailable(version) => {
                self.update_available = Some(version);
            }
        }
        self.recompose();
    }

    fn recompose(&mut self) {
        let next = compose(
            self.git.as_ref(),
            self.github.as_ref(),
            self.store.as_ref(),
            &self.config,
        );
        if self.state.stack_status == StackStatus::Ready {
            for cell in diff_cells(&self.state, &next) {
                self.animation.flash(cell, 5);
            }
        }
        self.state = next;
        self.sync_selection();
    }
}
