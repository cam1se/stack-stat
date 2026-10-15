//! Pure composition of provider snapshots into terminal-facing state.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::{
    config::Config,
    model::{
        BranchState, CheckConclusion, CheckStatus, CiCheck, LocalCheckResult, LocalCheckState,
        LocalSync, PrInfo, PrLifecycle, RebaseNeed, Review, ReviewDecision, ReviewState,
        StackState, StackStatus,
    },
    providers::{PrResponse, ReportedState, StackDetailsResponse, StackViewResponse, StatusEntry},
};

/// Successful local Git poll result.
#[derive(Clone, Debug)]
pub struct GitSnapshot {
    /// Checked-out worktree branch.
    pub current_branch: String,
    /// Current branch's state relative to its remote-tracking ref.
    pub current_sync: LocalSync,
    /// Local branch to full tip SHA.
    pub branch_shas: HashMap<String, String>,
    /// Poll completion time.
    pub fetched_at: DateTime<Utc>,
}

/// Normalized GitHub discovery outcome, separate from transport failures.
#[derive(Clone, Debug)]
pub enum StackOutcome {
    /// A stacked or single-mode view is available.
    InStack(StackViewResponse),
    /// No attached branch with a HEAD commit is available to display.
    NotInStack,
}

/// Successful GitHub poll result, including an unavailable-branch result.
#[derive(Clone, Debug)]
pub struct GitHubSnapshot {
    /// `owner/name` of the GitHub repository, empty when unknown.
    pub repo: String,
    /// Git branch this discovery ran for; empty when unknown.
    pub branch: String,
    /// Normalized stacked/single view or unavailable-branch outcome.
    pub stack: StackOutcome,
    /// Remote PR details and rebase comparisons for this view.
    pub details: StackDetailsResponse,
    /// Poll completion time.
    pub fetched_at: DateTime<Utc>,
}

/// Successful local check-store poll result.
#[derive(Clone, Debug)]
pub struct StoreSnapshot {
    /// Available reports by branch and SHA.
    pub entries: Vec<StatusEntry>,
    /// Malformed or unreadable file diagnostics.
    pub warnings: Vec<String>,
    /// Poll completion time.
    pub fetched_at: DateTime<Utc>,
}

/// Merge whichever source slices have arrived without performing I/O.
#[must_use]
pub fn compose(
    git: Option<&GitSnapshot>,
    github: Option<&GitHubSnapshot>,
    store: Option<&StoreSnapshot>,
    config: &Config,
) -> StackState {
    let mut state = StackState {
        repo: github.map_or_else(String::new, |github| github.repo.clone()),
        worktree_branch: git.map_or_else(String::new, |git| git.current_branch.clone()),
        github_branch: github.map_or_else(String::new, |github| github.branch.clone()),
        last_git_fetch: git.map(|snapshot| snapshot.fetched_at),
        last_github_fetch: github.map(|snapshot| snapshot.fetched_at),
        last_store_fetch: store.map(|snapshot| snapshot.fetched_at),
        ..StackState::default()
    };
    let Some(github) = github else {
        return state;
    };
    let StackOutcome::InStack(stack) = &github.stack else {
        state.stack_status = StackStatus::NotInStack;
        return state;
    };
    state.stack_status = StackStatus::Ready;
    state.mode = stack.mode;
    state.trunk = stack.trunk.clone();
    let prs: HashMap<_, _> = github
        .details
        .prs
        .iter()
        .map(|pr| (pr.number, pr))
        .collect();
    let mut found_non_merged_layer = false;
    let mut rebase_cascading = false;
    for branch in &stack.branches {
        let head_sha = git
            .and_then(|git| git.branch_shas.get(&branch.name))
            .cloned()
            .unwrap_or_default();
        let (is_current, local_sync) = match git {
            Some(git) if git.current_branch == branch.name => (true, git.current_sync),
            _ => (false, LocalSync::InSync),
        };
        let pr_number = branch.pr.as_ref().map(|pr| pr.number);
        let pr = pr_number
            .and_then(|number| prs.get(&number))
            .map(|pr| compose_pr(pr, config));
        let lifecycle = if branch.is_merged {
            PrLifecycle::Merged
        } else if branch.is_queued {
            PrLifecycle::Queued
        } else {
            PrLifecycle::Open
        };
        let own_rebase =
            if branch.is_merged || !github.details.remote_rebase_branches.contains(&branch.name) {
                RebaseNeed::None
            } else if found_non_merged_layer {
                RebaseNeed::Parent
            } else {
                RebaseNeed::Trunk
            };
        let rebase = match own_rebase {
            RebaseNeed::None | RebaseNeed::Trunk if rebase_cascading && !branch.is_merged => {
                RebaseNeed::Stacked
            }
            other => other,
        };
        rebase_cascading |= own_rebase == RebaseNeed::Parent
            || (own_rebase == RebaseNeed::Trunk && config.rebase.onto_trunk);
        found_non_merged_layer |= !branch.is_merged;
        state.branches.push(BranchState {
            name: branch.name.clone(),
            head_sha: head_sha.clone(),
            rebase,
            trunk_rebase_gates: config.rebase.onto_trunk,
            is_current,
            local_sync,
            lifecycle,
            pr_number,
            pr,
            local_checks: compose_local(&branch.name, &head_sha, store, config),
        });
    }
    state
}

fn compose_pr(pr: &PrResponse, config: &Config) -> PrInfo {
    let checks = pr
        .checks
        .iter()
        .map(|check| CiCheck {
            name: check.name.clone(),
            workflow: check.workflow.clone(),
            status: match check.status.as_str() {
                "COMPLETED" => CheckStatus::Completed,
                "QUEUED" | "PENDING" => CheckStatus::Queued,
                _ => CheckStatus::InProgress,
            },
            conclusion: check.conclusion.as_deref().map(|value| match value {
                "SUCCESS" => CheckConclusion::Success,
                "FAILURE" | "ERROR" | "STARTUP_FAILURE" => CheckConclusion::Failure,
                "SKIPPED" => CheckConclusion::Skipped,
                "CANCELLED" => CheckConclusion::Cancelled,
                "TIMED_OUT" => CheckConclusion::TimedOut,
                "ACTION_REQUIRED" => CheckConclusion::ActionRequired,
                "NEUTRAL" => CheckConclusion::Neutral,
                _ => CheckConclusion::Failure,
            }),
            started_at: check.started_at,
            completed_at: check.completed_at,
            details_url: check.details_url.clone(),
            skipped_by_config: config.is_check_skipped(&check.name),
        })
        .collect();
    let mut reviews: Vec<Review> = pr
        .reviews
        .iter()
        .map(|review| Review {
            author: review.author.clone(),
            state: match review.state.as_str() {
                "APPROVED" => ReviewState::Approved,
                "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
                "DISMISSED" => ReviewState::Dismissed,
                _ => ReviewState::Commented,
            },
            is_bot: review.is_bot || config.is_bot(&review.author),
        })
        .collect();
    for request in &pr.review_requests {
        if !reviews.iter().any(|review| review.author == request.author) {
            reviews.push(Review {
                author: request.author.clone(),
                state: ReviewState::Pending,
                is_bot: request.is_bot || config.is_bot(&request.author),
            });
        }
    }
    PrInfo {
        number: pr.number,
        url: pr.url.clone(),
        title: pr.title.clone(),
        is_draft: pr.is_draft,
        review_decision: match pr.review_decision.as_deref() {
            Some("APPROVED") => ReviewDecision::Approved,
            Some("CHANGES_REQUESTED") => ReviewDecision::ChangesRequested,
            Some("REVIEW_REQUIRED") => ReviewDecision::ReviewRequired,
            _ => ReviewDecision::None,
        },
        unresolved_threads: pr.unresolved_threads,
        threads_gate: config.reviews.gate_on_unresolved_threads,
        checks,
        reviews,
    }
}

fn compose_local(
    branch: &str,
    head_sha: &str,
    store: Option<&StoreSnapshot>,
    config: &Config,
) -> Vec<LocalCheckResult> {
    let entry = store.and_then(|store| {
        let same_branch = store.entries.iter().filter(|entry| entry.branch == branch);
        same_branch
            .clone()
            .find(|entry| !head_sha.is_empty() && entry.sha == head_sha)
            .or_else(|| {
                same_branch
                    .max_by_key(|entry| entry.checks.values().map(|check| check.recorded_at).max())
            })
    });
    let mut checks = Vec::new();
    for name in config.checks.local.keys() {
        checks.push(compose_check(name, entry, head_sha));
    }
    if let Some(entry) = entry.filter(|_| !head_sha.is_empty()) {
        for name in entry.checks.keys() {
            if !config.checks.local.contains_key(name) {
                checks.push(compose_check(name, Some(entry), head_sha));
            }
        }
    }
    checks
}

fn compose_check(name: &str, entry: Option<&StatusEntry>, head_sha: &str) -> LocalCheckResult {
    let report = entry
        .filter(|_| !head_sha.is_empty())
        .and_then(|entry| entry.checks.get(name));
    LocalCheckResult {
        name: name.into(),
        state: match report.map(|report| report.state) {
            Some(ReportedState::Pass) => LocalCheckState::Pass,
            Some(ReportedState::Fail) => LocalCheckState::Fail,
            Some(ReportedState::Skip) => LocalCheckState::Skip,
            None => LocalCheckState::Missing,
        },
        sha: report
            .and(entry)
            .map_or_else(String::new, |entry| entry.sha.clone()),
        is_stale: report.is_some_and(|_| entry.is_some_and(|entry| entry.sha != head_sha)),
        message: report.and_then(|report| report.message.clone()),
        recorded_at: report.map(|report| report.recorded_at),
    }
}
