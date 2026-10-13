//! Messages from pollers and effects emitted by App input handling.

use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StoreSnapshot},
    providers::{GitHubError, ProviderAction},
};

/// Independent data source refreshed by pollers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Local Git worktree and refs.
    Git,
    /// GitHub stack and PR state.
    GitHub,
    /// Local check status store.
    Store,
    /// Every data source, for forced refresh effects.
    All,
}

/// One requested action, optionally preceded by a confirmed local-link checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionSteps {
    /// Setup action that must succeed before the requested action runs.
    pub before: Option<ProviderAction>,
    /// User-requested action whose result completes the operation.
    pub action: ProviderAction,
}

impl ActionSteps {
    /// Plan a single action without a local-link step.
    #[must_use]
    pub fn single(action: ProviderAction) -> Self {
        Self {
            before: None,
            action,
        }
    }

    /// Check out the GitHub stack target, then run the requested action.
    #[must_use]
    pub fn linked(target: String, action: ProviderAction) -> Self {
        Self {
            before: Some(ProviderAction::Checkout(target)),
            action,
        }
    }

    /// Iterate over the exact confirmed actions in execution order.
    pub fn iter(&self) -> impl Iterator<Item = &ProviderAction> {
        self.before.iter().chain(std::iter::once(&self.action))
    }
}

/// Event delivered to the App by an I/O task or initial version check.
#[derive(Debug)]
pub enum AppMessage {
    /// Local Git state, or an error preserving the previous snapshot.
    Git(Result<GitSnapshot, String>),
    /// GitHub state, or classified fetch error.
    GitHub(Result<GitHubSnapshot, GitHubError>),
    /// Local check store state, or an error preserving the previous snapshot.
    Store(Result<StoreSnapshot, String>),
    /// A source began refreshing.
    FetchStarted(Source),
    /// A command effect completed.
    MutationDone {
        /// Exact confirmed steps, including a local-link checkout when needed.
        steps: ActionSteps,
        /// Success message or classified failure.
        result: Result<String, GitHubError>,
    },
    /// A newer stable release is available.
    UpdateAvailable(String),
}

/// Side effect requested by App, executed by the runtime or directly in tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Trigger immediate source refresh.
    Refresh(Source),
    /// Perform confirmed GitHub actions in order, aborting on the first failure.
    Mutate(ActionSteps),
    /// Restore the terminal and quit.
    Quit,
}
