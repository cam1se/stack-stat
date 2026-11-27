//! Cheap change detection that runs in front of the GraphQL stack query.

use async_trait::async_trait;

use super::GitHubError;

/// Result of one conditional-request pass over the watched pull requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeOutcome {
    /// Whether any watched resource differs from the previous pass.
    ///
    /// A resource seen for the first time counts as changed.
    pub changed: bool,
    /// Requests answered `304 Not Modified`, which GitHub does not bill.
    pub not_modified: u32,
    /// Requests answered with a fresh body.
    pub modified: u32,
}

/// Detects that GitHub state for some pull requests changed, without spending GraphQL points.
///
/// Implementations remember what they saw on the previous call, so the first call for a
/// pull request always reports a change.
#[async_trait]
pub trait ChangeProbe: Send + Sync {
    /// Compare the watched pull requests with the previous pass and remember the new state.
    async fn probe(&self, prs: &[u32]) -> Result<ProbeOutcome, GitHubError>;
}

/// Map an unexpected REST status to a typed error.
///
/// `remaining` and `reset` are the `X-RateLimit-Remaining` and `X-RateLimit-Reset` headers.
/// A 403 or 429 is a rate limit only when GitHub says the budget is exhausted or asks the
/// client to retry; other 403 responses are permission failures.
#[must_use]
pub fn classify_rest_status(
    status: u16,
    remaining: Option<&str>,
    reset: Option<&str>,
    retry_after: Option<&str>,
) -> GitHubError {
    let exhausted = remaining.is_some_and(|value| value.trim() == "0");
    if status == 429 || (status == 403 && (exhausted || retry_after.is_some())) {
        let hint = reset
            .and_then(|value| value.trim().parse::<i64>().ok())
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| format!("resets at {}", time.to_rfc3339()));
        return GitHubError::RateLimited { reset_hint: hint };
    }
    match status {
        401 => GitHubError::Auth,
        _ => GitHubError::Network(format!("GitHub REST returned HTTP {status}")),
    }
}
