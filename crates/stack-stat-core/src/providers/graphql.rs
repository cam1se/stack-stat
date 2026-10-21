//! Pure GraphQL query generation, response parsing, and gh error classification.

use super::{GitHubError, GraphqlResponse, StackDetailsResponse, StackViewResponse};

mod stack_discovery;
pub use stack_discovery::StackDiscovery;

/// Build aliased PR and remote parent-comparison selections for a stack.
#[must_use]
pub fn build_stack_query(owner: &str, repo: &str, stack: &StackViewResponse) -> String {
    let mut numbers: Vec<_> = stack
        .branches
        .iter()
        .filter_map(|branch| branch.pr.as_ref().map(|pr| pr.number))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    let owner = serde_json::to_string(owner).expect("serializing an owned string cannot fail");
    let repo = serde_json::to_string(repo).expect("serializing an owned string cannot fail");
    let mut query = format!("query {{ repository(owner:{owner}, name:{repo}) {{ ");
    const FIELDS: &str = "number url title isDraft reviewDecision \
        latestReviews(first:50) { nodes { state submittedAt author { login __typename } } } \
        reviewRequests(first:50) { nodes { requestedReviewer { __typename ... on User { login } ... on Bot { login } ... on Team { name } } } } \
        reviewThreads(first:100) { nodes { isResolved } } \
        commits(last:1) { nodes { commit { statusCheckRollup { contexts(first:100) { nodes { __typename \
        ... on CheckRun { name status conclusion startedAt completedAt detailsUrl checkSuite { workflowRun { runNumber workflow { name } } } } \
        ... on StatusContext { context state targetUrl createdAt } } } } } } }";
    for number in numbers {
        query.push_str(&format!(
            "pr{number}: pullRequest(number: {number}) {{ {FIELDS} }} "
        ));
    }
    let mut parent = stack.trunk.as_str();
    for (index, branch) in stack
        .branches
        .iter()
        .filter(|branch| !branch.is_merged)
        .enumerate()
    {
        let parent_ref = serde_json::to_string(&format!("refs/heads/{parent}"))
            .expect("serializing an owned string cannot fail");
        let child_ref =
            serde_json::to_string(&branch.name).expect("serializing an owned string cannot fail");
        query.push_str(&format!(
            "rebase{index}: ref(qualifiedName:{parent_ref}) {{ compare(headRef:{child_ref}) {{ behindBy }} }} "
        ));
        parent = &branch.name;
    }
    query.push_str("} }");
    query
}

/// Parse an aliased stack query response while retaining available partial data.
pub fn parse_stack_response(
    raw: &str,
    stack: &StackViewResponse,
) -> Result<StackDetailsResponse, GitHubError> {
    let response: GraphqlResponse =
        serde_json::from_str(raw).map_err(|error| GitHubError::Parse(error.to_string()))?;
    if response
        .data
        .as_ref()
        .and_then(|data| data.repository.as_ref())
        .is_none()
    {
        let detail = response
            .errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(GitHubError::Parse(if detail.is_empty() {
            "GraphQL response had no repository data".into()
        } else {
            detail
        }));
    }
    Ok(response.into_stack_details(stack))
}

/// Classify a nonzero gh exit into an actionable error without relying on stdout.
#[must_use]
pub fn classify_gh_failure(args: &[&str], code: i32, stderr: &str) -> GitHubError {
    let lower = stderr.to_ascii_lowercase();
    if args.first() == Some(&"stack") {
        if lower.contains("unknown command \"stack\" for \"gh\"") {
            return GitHubError::MissingStackExtension;
        }
        if code == 2
            && matches!(args.get(1), Some(&"push" | &"sync" | &"rebase"))
            && lower.contains("is not part of a stack")
        {
            return GitHubError::LocalTrackingMissing;
        }
        if matches!(code, 3 | 7) {
            return GitHubError::RebaseConflict;
        }
    }
    if lower.contains("rate limit") {
        let hint = stderr
            .lines()
            .find(|line| {
                let lower = line.to_ascii_lowercase();
                lower.contains("resets at") || lower.contains("retry after")
            })
            .map(str::to_owned);
        return GitHubError::RateLimited { reset_hint: hint };
    }
    if code == 4
        || lower.contains("auth")
        || lower.contains("gh auth login")
        || lower.contains("bad credentials")
    {
        return GitHubError::Auth;
    }
    if lower.contains("connection")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("network")
    {
        return GitHubError::Network(stderr.trim().into());
    }
    GitHubError::Command {
        code,
        stderr: stderr.trim().into(),
    }
}
