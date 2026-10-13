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
    /// Stack and PR state through gh.
    GitHub,
    /// Local check status store.
    Store,
    /// Every data source, for forced refresh effects.
    All,
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
        /// Attempted operation.
        action: ProviderAction,
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
    /// Perform a GitHub action.
    Mutate(ProviderAction),
    /// Restore the terminal and quit.
    Quit,
}
