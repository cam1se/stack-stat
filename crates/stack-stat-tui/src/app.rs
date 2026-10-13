//! Application state: successful snapshots, selection, modes, and feedback.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StoreSnapshot, compose},
    config::{Config, ReviewsDefault},
    model::{StackState, StackStatus},
    providers::{GitHubError, GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider},
};

use crate::{
    message::{AppMessage, Source},
    ui::animation::{AnimationState, diff_cells},
};

mod effects;
mod keys;
mod state;
pub use effects::execute;

/// Current input context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Branch table and global commands.
    Normal,
    /// Incremental branch-name search.
    Filter,
    /// Awaiting an explicit y/n response for a mutation.
    Confirm(ProviderAction),
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
    /// Extension-provided trunk-to-top order.
    #[default]
    Stack,
    /// Numeric PR number, ascending.
    PrNumber,
    /// Worst check status first, then stack order.
    Status,
}

/// Current feedback in the footer.
#[derive(Clone, Debug, Default)]
pub struct StatusLine {
    /// Text visible to the user.
    pub text: String,
    /// Whether the text reports a failure.
    pub is_error: bool,
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
    status: StatusLine,
    errors: HashMap<Source, String>,
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
        let status = StatusLine {
            text: config.warnings.first().cloned().unwrap_or_default(),
            is_error: false,
        };
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
            mutation_busy: false,
            fetching: HashSet::new(),
            errors: HashMap::new(),
            now: Utc::now(),
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
                        if let Some(warning) = snapshot.warnings.first() {
                            self.error(Source::Store, format!("! {warning}"));
                        } else {
                            self.clear_error(Source::Store);
                        }
                        self.store = Some(snapshot);
                    }
                    Err(error) => self.error(Source::Store, format!("! local checks: {error}")),
                }
            }
            AppMessage::FetchStarted(source) => {
                self.fetching.insert(source);
            }
            AppMessage::MutationDone { action, result } => {
                if !matches!(action, ProviderAction::OpenPr(_)) {
                    self.mutation_busy = false;
                }
                match result {
                    Ok(output) => {
                        let text = if output.trim().is_empty() {
                            success_message(&action)
                        } else {
                            output
                                .lines()
                                .rev()
                                .find(|line| !line.is_empty())
                                .unwrap_or_default()
                                .into()
                        };
                        self.status = StatusLine {
                            text,
                            is_error: false,
                        };
                    }
                    Err(GitHubError::RebaseConflict) => self.status = StatusLine {
                        text:
                            "! rebase conflict — resolve with gh stack rebase --continue or --abort"
                                .into(),
                        is_error: true,
                    },
                    Err(error) => {
                        self.status = StatusLine {
                            text: format!(
                                "! {}",
                                error.to_string().lines().next().unwrap_or_default()
                            ),
                            is_error: true,
                        }
                    }
                }
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

    fn clear_error(&mut self, source: Source) {
        self.errors.remove(&source);
        if self.status.is_error {
            self.show_error_if_any();
        }
    }

    fn error(&mut self, source: Source, text: String) {
        self.errors.insert(source, text);
        if !matches!(self.mode, Mode::Confirm(_)) {
            self.show_error_if_any();
        }
    }

    fn show_error_if_any(&mut self) {
        if let Some(text) = [Source::GitHub, Source::Git, Source::Store]
            .iter()
            .find_map(|source| self.errors.get(source))
        {
            self.status = StatusLine {
                text: text.clone(),
                is_error: true,
            };
        } else if self.status.is_error {
            self.status = StatusLine::default();
        }
    }
}

fn success_message(action: &ProviderAction) -> String {
    match action {
        ProviderAction::OpenPr(number) => format!("Opened PR #{number}"),
        ProviderAction::Checkout(branch) => format!("Checked out {branch}"),
        ProviderAction::PushStack => "Pushed stack".into(),
        ProviderAction::SyncStack => "Synced stack".into(),
        ProviderAction::ToggleDraft { number, make_ready } => {
            format!(
                "PR #{number} is {}",
                if *make_ready { "ready" } else { "draft" }
            )
        }
    }
}
