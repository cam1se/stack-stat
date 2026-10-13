//! Serialized external records and typed provider failures.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Result from `gh stack view --json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StackViewResponse {
    /// Trunk branch name.
    pub trunk: String,
    /// Checked-out branch according to the extension.
    pub current_branch: String,
    /// Layers from trunk to top.
    pub branches: Vec<StackBranch>,
}

/// One layer of a gh-stack view.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StackBranch {
    /// Local branch name.
    pub name: String,
    /// Commit at the branch tip.
    pub head: String,
    /// Recorded parent base commit.
    pub base: String,
    /// Whether the extension considers this branch checked out.
    pub is_current: bool,
    /// Whether the layer has merged.
    pub is_merged: bool,
    /// Whether it is queued for merge.
    pub is_queued: bool,
    /// Whether the parent moved since the last rebase.
    pub needs_rebase: bool,
    /// Linked PR, if one exists.
    pub pr: Option<StackPr>,
}

/// Linked PR identity in a stack view.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct StackPr {
    /// GitHub PR number.
    pub number: u32,
    /// PR URL.
    pub url: String,
    /// Extension-provided lifecycle state.
    pub state: String,
}

/// PR details normalized from an aliased GraphQL response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrResponse {
    /// PR number.
    pub number: u32,
    /// URL to the PR.
    pub url: String,
    /// Whether the PR is a draft.
    pub is_draft: bool,
    /// GitHub aggregate decision; absent when no decision exists.
    pub review_decision: Option<String>,
    /// Latest submitted review from each reviewer.
    pub reviews: Vec<ReviewResponse>,
    /// Requested reviewers who have not necessarily submitted a review.
    pub review_requests: Vec<ReviewRequest>,
    /// Normalized check runs and status contexts for the head commit.
    pub checks: Vec<PrCheckResponse>,
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

/// Classified GitHub CLI failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitHubError {
    /// The checked-out branch is not in a stack.
    NotInStack,
    /// A rebase needs manual conflict resolution.
    RebaseConflict,
    /// API request quota is exhausted.
    RateLimited {
        /// Optional reset hint supplied by GitHub.
        reset_hint: Option<String>,
    },
    /// Authentication is missing or expired.
    Auth,
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
            Self::RebaseConflict => write!(f, "stack rebase conflict"),
            Self::RateLimited { reset_hint } => write!(
                f,
                "GitHub rate limited{}",
                reset_hint.as_deref().unwrap_or("")
            ),
            Self::Auth => write!(f, "GitHub CLI authentication failed"),
            Self::Network(msg) | Self::Parse(msg) => write!(f, "{msg}"),
            Self::Command { code, stderr } => write!(f, "gh exited {code}: {stderr}"),
        }
    }
}
impl std::error::Error for GitHubError {}
