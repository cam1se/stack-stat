//! Aliased GitHub GraphQL response and normalization.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{PrCheckResponse, PrResponse, ReviewRequest, ReviewResponse};
/// Top-level aliased `gh api graphql` reply.
#[derive(Clone, Debug, Deserialize)]
pub struct GraphqlResponse {
    /// Returned repository and PRs; may be partial when `errors` is nonempty.
    pub data: Option<GraphqlData>,
    /// GraphQL errors for missing or inaccessible fields.
    #[serde(default)]
    pub errors: Vec<GraphqlError>,
}

/// One GraphQL error message.
#[derive(Clone, Debug, Deserialize)]
pub struct GraphqlError {
    /// GitHub's diagnostic.
    pub message: String,
}

/// GraphQL data wrapper.
#[derive(Clone, Debug, Deserialize)]
pub struct GraphqlData {
    /// Repository under which PR aliases were queried.
    pub repository: Option<GraphqlRepository>,
}

/// Aliased PR nodes keyed by query alias.
#[derive(Clone, Debug, Deserialize)]
pub struct GraphqlRepository {
    /// Alias to PR (null for deleted or inaccessible PRs).
    #[serde(flatten)]
    pub prs: BTreeMap<String, Option<RawPr>>,
}

/// Raw GraphQL fields for one PR.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPr {
    /// PR number.
    pub number: u32,
    /// PR URL.
    pub url: String,
    /// PR title.
    #[serde(default)]
    pub title: String,
    /// Draft state.
    pub is_draft: bool,
    /// Optional aggregate review decision.
    pub review_decision: Option<String>,
    /// Latest submitted reviews.
    pub latest_reviews: Option<Nodes<RawReview>>,
    /// Current review requests.
    pub review_requests: Option<Nodes<RawRequest>>,
    /// Review threads returned by GitHub.
    #[serde(default)]
    pub review_threads: Option<Nodes<RawReviewThread>>,
    /// Last commit with check rollup.
    pub commits: Option<Nodes<RawCommitNode>>,
}

/// GraphQL connection payload.
#[derive(Clone, Debug, Deserialize)]
pub struct Nodes<T> {
    /// Connection nodes, including nullable ones.
    pub nodes: Vec<Option<T>>,
}

/// Raw author login and GraphQL type.
#[derive(Clone, Debug, Deserialize)]
pub struct RawAuthor {
    /// GitHub login, when applicable.
    pub login: Option<String>,
    /// Team name, when the reviewer is a team.
    pub name: Option<String>,
    /// GraphQL type such as User, Bot, or Team.
    #[serde(rename = "__typename")]
    pub typename: String,
}

/// Raw submitted review.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawReview {
    /// Review state.
    pub state: String,
    /// Submission timestamp.
    pub submitted_at: Option<DateTime<Utc>>,
    /// Reviewer details; deleted accounts may have no author.
    pub author: Option<RawAuthor>,
}

/// Raw review request.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRequest {
    /// Requested user, bot, or team.
    pub requested_reviewer: Option<RawAuthor>,
}

/// Raw review-thread resolution state.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawReviewThread {
    /// Whether all comments in the thread have been resolved.
    pub is_resolved: bool,
}

/// Commit connection node.
#[derive(Clone, Debug, Deserialize)]
pub struct RawCommitNode {
    /// Git commit metadata.
    pub commit: RawCommit,
}

/// Git commit check rollup container.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCommit {
    /// Combined check rollup, possibly absent.
    pub status_check_rollup: Option<RawRollup>,
}

/// Status rollup contexts.
#[derive(Clone, Debug, Deserialize)]
pub struct RawRollup {
    /// Check runs and status contexts.
    pub contexts: Nodes<RawCheck>,
}

/// A check run or legacy commit status in the GraphQL union.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "__typename")]
pub enum RawCheck {
    /// GitHub Actions check run.
    CheckRun {
        /// Job name.
        name: String,
        /// Execution status.
        status: String,
        /// Conclusion if completed.
        conclusion: Option<String>,
        /// Start time.
        #[serde(rename = "startedAt")]
        started_at: Option<DateTime<Utc>>,
        /// Completion time.
        #[serde(rename = "completedAt")]
        completed_at: Option<DateTime<Utc>>,
        /// Details URL.
        #[serde(rename = "detailsUrl")]
        details_url: Option<String>,
        /// Workflow metadata, when available.
        #[serde(rename = "checkSuite")]
        check_suite: Option<RawCheckSuite>,
    },
    /// Commit status context.
    StatusContext {
        /// Context name.
        context: String,
        /// Legacy state such as SUCCESS, FAILURE, PENDING.
        state: String,
        /// Details URL.
        #[serde(rename = "targetUrl")]
        target_url: Option<String>,
        /// Creation timestamp.
        #[serde(rename = "createdAt")]
        created_at: Option<DateTime<Utc>>,
    },
}

/// Check suite's workflow link.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCheckSuite {
    /// Workflow run if this check belongs to one.
    pub workflow_run: Option<RawWorkflowRun>,
}

/// Workflow run metadata.
#[derive(Clone, Debug, Deserialize)]
pub struct RawWorkflowRun {
    /// Workflow identity.
    pub workflow: Option<RawWorkflow>,
}

/// Workflow name in a check suite.
#[derive(Clone, Debug, Deserialize)]
pub struct RawWorkflow {
    /// Human-readable workflow name.
    pub name: String,
}

impl GraphqlResponse {
    /// Normalize available PRs; missing/deleted aliased PRs are omitted.
    #[must_use]
    pub fn into_prs(self) -> Vec<PrResponse> {
        let Some(repository) = self.data.and_then(|data| data.repository) else {
            return Vec::new();
        };
        repository
            .prs
            .into_values()
            .flatten()
            .map(RawPr::normalize)
            .collect()
    }
}

impl RawPr {
    fn normalize(self) -> PrResponse {
        let reviews = self
            .latest_reviews
            .into_iter()
            .flat_map(|value| value.nodes)
            .flatten()
            .filter_map(|review| {
                let author = review.author?;
                Some(ReviewResponse {
                    author: author.login.or(author.name)?,
                    state: review.state,
                    is_bot: author.typename == "Bot",
                    submitted_at: review.submitted_at,
                })
            })
            .collect();
        let review_requests = self
            .review_requests
            .into_iter()
            .flat_map(|value| value.nodes)
            .flatten()
            .filter_map(|request| {
                let reviewer = request.requested_reviewer?;
                Some(ReviewRequest {
                    author: reviewer.login.or(reviewer.name)?,
                    is_bot: reviewer.typename == "Bot",
                })
            })
            .collect();
        let unresolved_threads = self
            .review_threads
            .into_iter()
            .flat_map(|value| value.nodes)
            .flatten()
            .filter(|thread| !thread.is_resolved)
            .count();
        let checks = self
            .commits
            .into_iter()
            .flat_map(|value| value.nodes)
            .flatten()
            .filter_map(|node| node.commit.status_check_rollup)
            .flat_map(|rollup| rollup.contexts.nodes)
            .flatten()
            .map(RawCheck::normalize)
            .collect();
        PrResponse {
            number: self.number,
            url: self.url,
            title: self.title,
            is_draft: self.is_draft,
            review_decision: self.review_decision,
            reviews,
            review_requests,
            unresolved_threads,
            checks,
        }
    }
}

impl RawCheck {
    fn normalize(self) -> PrCheckResponse {
        match self {
            Self::CheckRun {
                name,
                status,
                conclusion,
                started_at,
                completed_at,
                details_url,
                check_suite,
            } => {
                let workflow = check_suite
                    .and_then(|suite| suite.workflow_run)
                    .and_then(|run| run.workflow)
                    .map_or_else(String::new, |workflow| workflow.name);
                PrCheckResponse {
                    name,
                    workflow,
                    status,
                    conclusion,
                    started_at,
                    completed_at,
                    details_url: details_url.unwrap_or_default(),
                }
            }
            Self::StatusContext {
                context,
                state,
                target_url,
                created_at,
            } => {
                let (status, conclusion) = if state == "PENDING" {
                    ("IN_PROGRESS".into(), None)
                } else {
                    ("COMPLETED".into(), Some(state))
                };
                PrCheckResponse {
                    name: context,
                    workflow: String::new(),
                    status,
                    conclusion,
                    started_at: created_at,
                    completed_at: None,
                    details_url: target_url.unwrap_or_default(),
                }
            }
        }
    }
}
