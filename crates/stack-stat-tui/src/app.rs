//! Application state: successful snapshots, selection, modes, and feedback.

use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Utc};
use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StoreSnapshot, compose},
    config::{Config, ReviewsDefault},
    model::StackState,
    providers::{GitHubError, GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider},
};

use crate::message::{AppMessage, Source};

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
    store: Option<StoreSnapshot>,
    state: StackState,
    selected_name: Option<String>,
    selected_index: usize,
    detail_open: bool,
    detail_scroll: usize,
    detail_viewport_rows: usize,
    mode: Mode,
    sort: SortOrder,
    filter: String,
    show_bots: bool,
    reviews_expanded: bool,
    status: StatusLine,
    mutation_busy: bool,
    fetching: HashSet<Source>,
    now: DateTime<Utc>,
    update_available: Option<String>,
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
            store: None,
            state: StackState::default(),
            selected_name: None,
            selected_index: 0,
            detail_open: false,
            detail_scroll: 0,
            detail_viewport_rows: 12,
            mode: Mode::Normal,
            sort: SortOrder::Stack,
            filter: String::new(),
            show_bots,
            reviews_expanded,
            status,
            mutation_busy: false,
            fetching: HashSet::new(),
            now: Utc::now(),
            update_available: None,
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
                        self.clear_error();
                    }
                    Err(error) => self.error(format!("⚠ git fetch failed: {error}")),
                }
            }
            AppMessage::GitHub(result) => {
                self.fetching.remove(&Source::GitHub);
                match result {
                    Ok(snapshot) => {
                        self.github = Some(snapshot);
                        self.clear_error();
                    }
                    Err(GitHubError::NotInStack) => {
                        self.github = Some(GitHubSnapshot {
                            stack: stack_stat_core::compose::StackOutcome::NotInStack,
                            prs: vec![],
                            fetched_at: self.now,
                        });
                        self.clear_error();
                    }
                    Err(GitHubError::RateLimited { reset_hint }) => self.error(format!(
                        "⚠ rate limited, resumes in {}",
                        reset_hint.unwrap_or_else(|| "5m".into())
                    )),
                    Err(error) => self.error(format!("⚠ fetch failed: {error}")),
                }
            }
            AppMessage::Store(result) => {
                self.fetching.remove(&Source::Store);
                match result {
                    Ok(snapshot) => {
                        if let Some(warning) = snapshot.warnings.first() {
                            self.status = StatusLine {
                                text: format!("⚠ {warning}"),
                                is_error: true,
                            };
                        } else {
                            self.clear_error();
                        }
                        self.store = Some(snapshot);
                    }
                    Err(error) => self.error(format!("⚠ local checks: {error}")),
                }
            }
            AppMessage::FetchStarted(source) => {
                self.fetching.insert(source);
            }
            AppMessage::MutationDone { action, result } => {
                self.mutation_busy = false;
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
                    Err(GitHubError::RebaseConflict) => self.error(
                        "⚠ rebase conflict — resolve with gh stack rebase --continue or --abort"
                            .into(),
                    ),
                    Err(error) => self.error(format!(
                        "⚠ {}",
                        error.to_string().lines().next().unwrap_or_default()
                    )),
                }
            }
            AppMessage::UpdateAvailable(version) => {
                self.update_available = Some(version);
            }
        }
        self.recompose();
    }

    fn recompose(&mut self) {
        self.state = compose(
            self.git.as_ref(),
            self.github.as_ref(),
            self.store.as_ref(),
            &self.config,
        );
        self.sync_selection();
    }

    fn clear_error(&mut self) {
        if self.status.is_error {
            self.status = StatusLine::default();
        }
    }

    fn error(&mut self, text: String) {
        self.status = StatusLine {
            text,
            is_error: true,
        };
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
