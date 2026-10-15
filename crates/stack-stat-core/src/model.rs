//! Domain state presented by the terminal dashboard.

use chrono::{DateTime, Utc};

mod mode;
pub use mode::StackMode;

/// A composed view of the active worktree's stack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackState {
    /// Whether GitHub supplied a real stack or a single branch view.
    pub mode: StackMode,
    /// Name of the stack's trunk branch, if known.
    pub trunk: String,
    /// `owner/name` of the GitHub repository, empty when unknown.
    pub repo: String,
    /// Branches in stack order, trunk-first.
    pub branches: Vec<BranchState>,
    /// Branch currently checked out in this worktree.
    pub worktree_branch: String,
    /// Branch the latest GitHub result was discovered for; empty when unknown.
    pub github_branch: String,
    /// Current state of stack discovery.
    pub stack_status: StackStatus,
    /// Last successful Git refresh.
    pub last_git_fetch: Option<DateTime<Utc>>,
    /// Last successful GitHub refresh.
    pub last_github_fetch: Option<DateTime<Utc>>,
    /// Last successful local-check refresh.
    pub last_store_fetch: Option<DateTime<Utc>>,
}

/// Outcome of finding the stack for the checked-out branch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackStatus {
    /// No GitHub response has arrived.
    #[default]
    Loading,
    /// The worktree has no attached branch to display.
    NotInStack,
    /// A stacked or single branch view is available.
    Ready,
}

/// A local branch's display state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchState {
    /// Branch name.
    pub name: String,
    /// Full HEAD commit identifier, empty when the local ref is absent.
    pub head_sha: String,
    /// Required rebase target derived from remote GitHub history.
    pub rebase: RebaseNeed,
    /// Whether trunk rebases block readiness.
    pub trunk_rebase_gates: bool,
    /// Whether this worktree has checked out this branch.
    pub is_current: bool,
    /// How the checked-out branch compares with its remote-tracking ref.
    pub local_sync: LocalSync,
    /// Its PR lifecycle state.
    pub lifecycle: PrLifecycle,
    /// PR number from stack metadata, even when GitHub details are unavailable.
    pub pr_number: Option<u32>,
    /// Remote PR details, if any.
    pub pr: Option<PrInfo>,
    /// Results reported for this branch, including missing configured checks.
    pub local_checks: Vec<LocalCheckResult>,
}

/// Local branch state relative to its remote-tracking ref.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocalSync {
    /// Local and remote-tracking refs name the same commit history.
    #[default]
    InSync,
    /// Local commits have not been pushed.
    Ahead,
    /// Remote commits have not been incorporated locally.
    Behind,
    /// Both local and remote have commits absent from the other.
    Diverged,
    /// The local branch has no remote-tracking ref.
    Unpushed,
}

/// Required remote rebase target for a stack layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RebaseNeed {
    /// No remote rebase is required.
    #[default]
    None,
    /// The remote parent ref has commits absent from this layer's remote head.
    Parent,
    /// The remote trunk ref has commits absent from this layer's remote head.
    Trunk,
    /// An earlier layer must be rebased, so this layer will move with it.
    Stacked,
}

/// GitHub's lifecycle status for a stack layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrLifecycle {
    /// The branch has an open PR, or no PR yet.
    Open,
    /// The PR has merged.
    Merged,
    /// The PR is in a merge queue.
    Queued,
    /// The PR was closed without merging.
    Closed,
}

/// Go/no-go readiness of a stack layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Ready to merge: nothing blocks and nothing is outstanding.
    Go,
    /// Not blocked, but evidence is still outstanding (running CI, pending review, missing result).
    Wait,
    /// Blocked by a draft, failure, requested changes, rebase, or stale/failed local result.
    NoGo,
    /// Already merged.
    Merged,
    /// In the merge queue.
    Queued,
    /// Closed without merging.
    Closed,
}

/// A concrete condition affecting a branch's readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    /// The branch has no pull request yet.
    NoPr,
    /// The pull request is still a draft.
    Draft,
    /// The branch must be rebased onto the layer below it.
    RebaseParent,
    /// The branch must be rebased onto trunk.
    RebaseTrunk,
    /// An earlier layer must be rebased, which will move this layer too.
    RebaseStacked,
    /// One or more CI checks failed.
    FailingChecks(usize),
    /// GitHub or a reviewer requested changes.
    ChangesRequested,
    /// One or more gated review threads remain unresolved.
    UnresolvedThreads(usize),
    /// One or more local checks failed.
    LocalFailed(usize),
    /// One or more local check results no longer match the branch head.
    LocalStale(usize),
    /// Pull request details have not been loaded.
    LoadingDetails,
    /// One or more CI checks are queued or running.
    RunningChecks(usize),
    /// GitHub reports that a review is required.
    ReviewRequired,
    /// A requested review remains pending.
    ReviewPending,
    /// One or more configured local checks have not reported a result.
    MissingLocal(usize),
}

impl Issue {
    /// Whether this issue prevents the branch from being merged.
    #[must_use]
    pub const fn is_blocking(self) -> bool {
        matches!(
            self,
            Self::Draft
                | Self::RebaseParent
                | Self::RebaseTrunk
                | Self::RebaseStacked
                | Self::FailingChecks(_)
                | Self::ChangesRequested
                | Self::UnresolvedThreads(_)
                | Self::LocalFailed(_)
                | Self::LocalStale(_)
        )
    }
}

/// Per-verdict branch counts for a whole stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StackSummary {
    /// Branches ready to merge.
    pub go: usize,
    /// Branches with outstanding evidence and no blocker.
    pub wait: usize,
    /// Blocked branches.
    pub no_go: usize,
    /// Merged layers.
    pub merged: usize,
    /// Layers in the merge queue.
    pub queued: usize,
    /// Layers whose PR was closed without merging.
    pub closed: usize,
}

/// Count of open layers with blockers in each category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockerSummary {
    /// Layers whose pull request is a draft.
    pub draft: usize,
    /// Layers requiring a rebase.
    pub rebase: usize,
    /// Layers with failed CI checks.
    pub checks: usize,
    /// Layers with requested or pending reviews.
    pub reviews: usize,
    /// Layers with unresolved gated threads.
    pub threads: usize,
    /// Layers with failed, stale, or missing local results.
    pub local: usize,
}

/// Pull request fields needed to render status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrInfo {
    /// GitHub PR number.
    pub number: u32,
    /// URL to the PR.
    pub url: String,
    /// PR title; empty when unknown.
    pub title: String,
    /// Whether the PR is a draft.
    pub is_draft: bool,
    /// Aggregate review decision supplied by GitHub.
    pub review_decision: ReviewDecision,
    /// Unresolved review threads among GitHub's first 100 returned threads.
    pub unresolved_threads: usize,
    /// Whether unresolved review threads block readiness.
    pub threads_gate: bool,
    /// CI checks on the head commit.
    pub checks: Vec<CiCheck>,
    /// Latest review from each author and pending review requests.
    pub reviews: Vec<Review>,
}

/// GitHub's aggregate review decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    /// Required reviewers approved.
    Approved,
    /// A reviewer requested changes.
    ChangesRequested,
    /// Review is still required.
    ReviewRequired,
    /// No decision is available.
    None,
}

/// One CI check or commit status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CiCheck {
    /// Check name.
    pub name: String,
    /// Workflow name, when GitHub supplies one.
    pub workflow: String,
    /// Execution status.
    pub status: CheckStatus,
    /// Completion result.
    pub conclusion: Option<CheckConclusion>,
    /// Start timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Completion timestamp.
    pub completed_at: Option<DateTime<Utc>>,
    /// URL to the detailed result.
    pub details_url: String,
    /// Whether the repository filters this check from the row summary.
    pub skipped_by_config: bool,
}

/// Execution stage for a CI check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckStatus {
    /// Waiting to run.
    Queued,
    /// Running now.
    InProgress,
    /// Finished (see conclusion).
    Completed,
}

/// Conclusion of a completed CI check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckConclusion {
    /// Check passed.
    Success,
    /// Check failed.
    Failure,
    /// Check was intentionally skipped.
    Skipped,
    /// Check was cancelled.
    Cancelled,
    /// Check timed out.
    TimedOut,
    /// Check requires intervention.
    ActionRequired,
    /// Check completed without passing or failing.
    Neutral,
}

/// Overall state of a row's visible checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckHealth {
    /// No visible checks exist.
    None,
    /// At least one check failed or could not complete successfully.
    Failure,
    /// At least one check remains queued or running.
    Running,
    /// Every counted check passed.
    Success,
    /// Only skipped or neutral checks exist.
    Skipped,
}

/// Count and worst state of checks not excluded by configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckSummary {
    /// Successfully completed checks.
    pub passed: usize,
    /// Counted, non-filtered checks.
    pub total: usize,
    /// Worst visible health, with failures taking precedence over running checks.
    pub worst: CheckHealth,
    /// Counted checks that are queued or running.
    pub running: usize,
}

/// A review by one author, or an outstanding review request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    /// Login or team name.
    pub author: String,
    /// Review status.
    pub state: ReviewState,
    /// Whether GitHub or repository configuration recognizes a bot.
    pub is_bot: bool,
}

/// Individual review outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewState {
    /// Review approved.
    Approved,
    /// Changes requested.
    ChangesRequested,
    /// Commented without approving.
    Commented,
    /// Review dismissed.
    Dismissed,
    /// Review requested but not submitted.
    Pending,
}

/// A collapsed, visible review state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewSummary {
    /// Changes are requested by a reviewer.
    ChangesRequested,
    /// At least one visible reviewer approved.
    Approved,
    /// Review remains outstanding.
    Pending,
    /// No actionable review is visible.
    None,
}

/// A named result submitted from a local agent, hook, or script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalCheckResult {
    /// Configured or reported check name.
    pub name: String,
    /// Result of running (or not running) the check.
    pub state: LocalCheckState,
    /// Commit identifier this result applies to; empty for never-run checks.
    pub sha: String,
    /// Whether the reported commit no longer matches branch HEAD.
    pub is_stale: bool,
    /// Optional explanatory text.
    pub message: Option<String>,
    /// When the result was recorded; absent for never-run checks.
    pub recorded_at: Option<DateTime<Utc>>,
}

/// Local check outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalCheckState {
    /// Check passed.
    Pass,
    /// Check failed.
    Fail,
    /// Check was explicitly skipped.
    Skip,
    /// Expected check has never been reported for this branch.
    Missing,
}

mod summary;
