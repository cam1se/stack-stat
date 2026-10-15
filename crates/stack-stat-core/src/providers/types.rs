//! Normalized external records and typed provider failures.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::StackMode;

/// GitHub stack or single branch structure normalized from discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackViewResponse {
    /// Whether this view represents real GitHub stack membership.
    pub mode: StackMode,
    /// Stack base, single PR base, or repository default branch for a no-PR row.
    pub trunk: String,
    /// Checked-out Git branch supplied by the caller.
    pub current_branch: String,
    /// Rows from trunk to top, ordered by stack-entry position in stacked mode.
    pub branches: Vec<StackBranch>,
}

/// One branch row, optionally linked to a GitHub pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackBranch {
    /// Branch name, retained from the PR even after its remote ref is deleted.
    pub name: String,
    /// Whether GitHub reports the PR as merged.
    pub is_merged: bool,
    /// Whether GitHub reports the PR in the merge queue.
    pub is_queued: bool,
    /// Linked PR, if one exists.
    pub pr: Option<StackPr>,
}

/// Linked GitHub PR identity and lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackPr {
    /// GitHub PR number.
    pub number: u32,
    /// PR URL.
    pub url: String,
    /// GitHub lifecycle state: `OPEN`, `MERGED`, or `CLOSED`.
    pub state: String,
}

/// PR details normalized from an aliased GraphQL response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrResponse {
    /// PR number.
    pub number: u32,
    /// URL to the PR.
    pub url: String,
    /// PR title; empty when GitHub did not supply one.
    pub title: String,
    /// Whether the PR is a draft.
    pub is_draft: bool,
    /// GitHub aggregate decision; absent when no decision exists.
    pub review_decision: Option<String>,
    /// Latest submitted review from each reviewer.
    pub reviews: Vec<ReviewResponse>,
    /// Requested reviewers who have not necessarily submitted a review.
    pub review_requests: Vec<ReviewRequest>,
    /// Unresolved review threads among the first 100 returned by GitHub.
    pub unresolved_threads: usize,
    /// Normalized check runs and status contexts for the head commit.
    pub checks: Vec<PrCheckResponse>,
}

/// Remote PR details and rebase comparisons for a stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackDetailsResponse {
    /// PR details for linked stack PRs.
    pub prs: Vec<PrResponse>,
    /// Non-merged branches whose remote heads are behind their remote parents.
    ///
    /// A branch is absent when its comparison is missing, failed, or has no
    /// commits behind its parent.
    pub remote_rebase_branches: BTreeSet<String>,
}

/// Individual GitHub review with author type preserved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewResponse {
    /// GitHub login.
    pub author: String,
    /// GitHub review state.
    pub state: String,
    /// Whether the author is of GraphQL type Bot.
    pub is_bot: bool,
    /// Review submission time.
    pub submitted_at: Option<DateTime<Utc>>,
}

/// Requested reviewer, which may be a person, bot, or team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRequest {
    /// Login or team name.
    pub author: String,
    /// Whether the request targets a bot.
    pub is_bot: bool,
}

/// A CI check run or commit status with shared fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrCheckResponse {
    /// Check name.
    pub name: String,
    /// Workflow name if known.
    pub workflow: String,
    /// GitHub execution status (or normalized state for a StatusContext).
    pub status: String,
    /// GitHub conclusion if completed.
    pub conclusion: Option<String>,
    /// Start time, if available.
    pub started_at: Option<DateTime<Utc>>,
    /// Completion time, if available.
    pub completed_at: Option<DateTime<Utc>>,
    /// Link to detailed logs.
    pub details_url: String,
}

mod graphql;
pub use graphql::*;

/// A locally reported check outcome.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReportedState {
    /// Passed.
    Pass,
    /// Failed.
    Fail,
    /// Deliberately skipped.
    Skip,
}

/// Stored result of one named local check.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CheckResult {
    /// Outcome of the check.
    pub state: ReportedState,
    /// Optional human-readable explanation.
    pub message: Option<String>,
    /// Time at which this outcome was recorded.
    pub recorded_at: DateTime<Utc>,
}

/// Statuses reported for a single branch at a specific commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusEntry {
    /// Local branch name.
    pub branch: String,
    /// Full commit SHA.
    pub sha: String,
    /// Check name to its most recently reported result.
    pub checks: BTreeMap<String, CheckResult>,
}

/// Observable effects recorded by the fake GitHub provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderAction {
    /// Open the selected PR in a browser.
    OpenPr(u32),
    /// Checkout a stack branch.
    Checkout(String),
    /// Push the current stack.
    PushStack,
    /// Push the current single-mode branch with plain Git.
    PushBranch,
    /// Synchronize with remote.
    SyncStack,
    /// Toggle draft state.
    ToggleDraft {
        /// PR number.
        number: u32,
        /// Whether the PR should become ready.
        make_ready: bool,
    },
}

/// Classified failure while discovering GitHub state or running a CLI action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitHubError {
    /// Nothing to display: no attached branch, or the repository default branch without a PR.
    NotInStack,
    /// Push or sync requires local gh-stack tracking before it can proceed.
    LocalTrackingMissing,
    /// A stack action requires the gh-stack extension to be installed.
    MissingStackExtension,
    /// A rebase needs manual conflict resolution.
    RebaseConflict,
    /// Sync stopped without changes because local and remote stacks diverged.
    Diverged(String),
    /// API request quota is exhausted.
    RateLimited {
        /// Optional reset hint supplied by GitHub.
        reset_hint: Option<String>,
    },
    /// Authentication is missing or expired.
    Auth,
    /// Local Git state needed for GitHub discovery could not be read.
    Git(String),
    /// Network operation failed.
    Network(String),
    /// Command returned an exit code and stderr.
    Command {
        /// Process exit code.
        code: i32,
        /// Human-readable stderr.
        stderr: String,
    },
    /// Response could not be interpreted.
    Parse(String),
}

impl std::fmt::Display for GitHubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInStack => write!(f, "not in a stack"),
            Self::LocalTrackingMissing => write!(f, "local gh-stack tracking missing"),
            Self::MissingStackExtension => write!(
                f,
                "gh-stack extension missing; install with: gh extension install github/gh-stack"
            ),
            Self::RebaseConflict => write!(f, "stack rebase conflict"),
            Self::Diverged(message) => write!(f, "Sync aborted: {message}"),
            Self::RateLimited { reset_hint } => write!(
                f,
                "GitHub rate limited{}",
                reset_hint.as_deref().unwrap_or("")
            ),
            Self::Auth => write!(f, "GitHub CLI authentication failed"),
            Self::Git(msg) | Self::Network(msg) | Self::Parse(msg) => write!(f, "{msg}"),
            Self::Command { code, stderr } => write!(f, "gh exited {code}: {stderr}"),
        }
    }
}
impl std::error::Error for GitHubError {}
