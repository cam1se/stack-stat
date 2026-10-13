//! Pure GraphQL query generation, response parsing, and gh error classification.

use super::{GitHubError, GraphqlResponse, PrResponse};

/// Build one aliased PR selection per distinct number in ascending order.
#[must_use]
pub fn build_pr_query(owner: &str, repo: &str, numbers: &[u32]) -> String {
    let mut sorted = numbers.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let owner = serde_json::to_string(owner).expect("serializing an owned string cannot fail");
    let repo = serde_json::to_string(repo).expect("serializing an owned string cannot fail");
    let mut query = format!("query {{ repository(owner:{owner}, name:{repo}) {{ ");
    const FIELDS: &str = "number url title isDraft reviewDecision \
        latestReviews(first:50) { nodes { state submittedAt author { login __typename } } } \
        reviewRequests(first:50) { nodes { requestedReviewer { __typename ... on User { login } ... on Bot { login } ... on Team { name } } } } \
        commits(last:1) { nodes { commit { statusCheckRollup { contexts(first:100) { nodes { __typename \
        ... on CheckRun { name status conclusion startedAt completedAt detailsUrl checkSuite { workflowRun { workflow { name } } } } \
        ... on StatusContext { context state targetUrl createdAt } } } } } } }";
    for number in sorted {
        query.push_str(&format!(
            "pr{number}: pullRequest(number: {number}) {{ {FIELDS} }} "
        ));
    }
    query.push_str("} }");
    query
}

/// Parse an aliased GraphQL response, keeping partial data when some PRs fail.
pub fn parse_pr_response(raw: &str) -> Result<Vec<PrResponse>, GitHubError> {
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
    Ok(response.into_prs())
}

/// Classify a nonzero gh exit into an actionable error without relying on stdout.
#[must_use]
pub fn classify_gh_failure(args: &[&str], code: i32, stderr: &str) -> GitHubError {
    if args.starts_with(&["stack", "view"]) && code == 2 {
        return GitHubError::NotInStack;
    }
    if args.first() == Some(&"stack") && matches!(code, 3 | 7) {
        return GitHubError::RebaseConflict;
    }
    let lower = stderr.to_ascii_lowercase();
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
    if lower.contains("auth")
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
