//! External Git, GitHub, and local status-store boundaries.

use std::{collections::HashMap, path::Path};

use anyhow::Result;
use async_trait::async_trait;

pub mod branch_dir;
mod file_store;
mod git_cli;
mod github_cli;
pub mod graphql;
pub use file_store::FileStatusStore;
pub use git_cli::{CliGitProvider, DETACHED_HEAD};
pub use github_cli::{CliGitHubProvider, classify_graphql_result, classify_sync_output};

mod retention;
mod types;
pub use retention::retained;
pub use types::*;

#[cfg(any(test, feature = "test-fakes"))]
mod fake_git;
#[cfg(any(test, feature = "test-fakes"))]
mod fake_github;
#[cfg(any(test, feature = "test-fakes"))]
mod memory_store;
#[cfg(any(test, feature = "test-fakes"))]
pub use fake_git::FakeGitProvider;
#[cfg(any(test, feature = "test-fakes"))]
pub use fake_github::FakeGitHubProvider;
#[cfg(any(test, feature = "test-fakes"))]
pub use memory_store::MemoryStatusStore;

/// Local git state needed by the dashboard.
#[async_trait]
pub trait GitProvider: Send + Sync {
    /// Checked-out branch name for the provider's worktree.
    async fn current_branch(&self) -> Result<String>;
    /// All local branch names and their full tip SHAs.
    async fn branch_shas(&self) -> Result<HashMap<String, String>>;
    /// Repository's git common directory shared by its worktrees.
    fn common_dir(&self) -> &Path;
}

/// GitHub stack and PR operations performed through `gh`.
#[async_trait]
pub trait GitHubProvider: Send + Sync {
    /// Fetch the stack containing the checked-out branch.
    async fn stack_view(&self) -> std::result::Result<StackViewResponse, GitHubError>;
    /// `owner/name` of the GitHub repository, empty when unknown.
    fn repo(&self) -> String;
    /// Fetch PR details by their numbers (not by open-PR listing).
    async fn pr_details(
        &self,
        numbers: &[u32],
    ) -> std::result::Result<Vec<PrResponse>, GitHubError>;
    /// Open a PR in a browser.
    async fn open_pr_in_browser(&self, number: u32) -> std::result::Result<(), GitHubError>;
    /// Checkout a branch in the stack.
    async fn checkout_branch(&self, branch: &str) -> std::result::Result<(), GitHubError>;
    /// Push the active stack branches.
    async fn push_stack(&self) -> std::result::Result<(), GitHubError>;
    /// Sync the stack with its remote state and return output.
    async fn sync_stack(&self) -> std::result::Result<String, GitHubError>;
    /// Make a PR ready or return it to draft.
    async fn toggle_draft(
        &self,
        number: u32,
        make_ready: bool,
    ) -> std::result::Result<(), GitHubError>;
}

/// Reported local check storage shared among worktrees.
pub trait StatusStoreProvider: Send + Sync {
    /// Read available entries (one per branch and SHA).
    fn read_all(&self) -> Result<Vec<StatusEntry>>;
    /// Read valid entries alongside warnings for malformed or unreadable files.
    fn read_with_warnings(&self) -> Result<(Vec<StatusEntry>, Vec<String>)> {
        self.read_all().map(|entries| (entries, Vec::new()))
    }
    /// Merge or replace one named check result for a branch and SHA.
    fn write(&self, branch: &str, sha: &str, check: &str, result: &CheckResult) -> Result<()>;
    /// Remove all entries for a branch (including stale entries).
    fn clear(&self, branch: &str) -> Result<()>;
    /// Keep the current and most recent stale entry per live branch; return removed count.
    fn gc(&self, live_branches: &HashMap<String, String>) -> Result<usize>;
}
