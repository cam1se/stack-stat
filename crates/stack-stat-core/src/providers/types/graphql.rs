//! Aliased GitHub GraphQL response and normalization.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
mod normalize;

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

/// Aliased PR and remote reference nodes returned by the query.
#[derive(Clone, Debug, Deserialize)]
pub struct GraphqlRepository {
    #[serde(flatten)]
    aliases: BTreeMap<String, Option<RawRepositoryAlias>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum RawRepositoryAlias {
    Pr(RawPr),
    Ref(RawReference),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawReference {
    #[serde(default)]
    compare: Option<RawComparison>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawComparison {
    #[serde(default)]
    behind_by: Option<u32>,
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
