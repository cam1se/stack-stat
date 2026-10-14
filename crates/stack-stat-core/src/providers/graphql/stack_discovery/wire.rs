//! Required stack-discovery fields and strict GraphQL failure handling.

use serde::{Deserialize, Deserializer, de::DeserializeOwned};

use crate::providers::{GitHubError, GraphqlError, graphql::classify_gh_failure};

#[derive(Debug, Deserialize)]
struct Response<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

pub(super) fn parse_response<T: DeserializeOwned>(
    code: i32,
    raw: &str,
    stderr: &str,
) -> Result<T, GitHubError> {
    let command_error = if code != 0 {
        let error = classify_gh_failure(&["api", "graphql"], code, stderr);
        if !matches!(error, GitHubError::Command { .. }) || raw.trim().is_empty() {
            return Err(error);
        }
        Some(error)
    } else {
        None
    };
    let response: Response<T> =
        serde_json::from_str(raw).map_err(|error| GitHubError::Parse(error.to_string()))?;
    if !response.errors.is_empty() {
        let mut detail = String::new();
        for error in response.errors {
            if !detail.is_empty() {
                detail.push_str("; ");
            }
            detail.push_str(&error.message);
        }
        return Err(
            match classify_gh_failure(&["api", "graphql"], code, &detail) {
                GitHubError::Command { .. } => GitHubError::Parse(detail),
                error => error,
            },
        );
    }
    if let Some(error) = command_error {
        return Err(error);
    }
    response
        .data
        .ok_or_else(|| GitHubError::Parse("GraphQL stack discovery response had no data".into()))
}

#[derive(Debug, Deserialize)]
pub(super) struct LookupData {
    pub repository: Option<LookupRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LookupRepository {
    pub default_branch_ref: Option<DefaultBranch>,
    pub open_pull_requests: Candidates,
    pub pull_requests: Candidates,
}

#[derive(Debug, Deserialize)]
pub(super) struct DefaultBranch {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Candidates {
    pub nodes: Vec<Option<Candidate>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Candidate {
    /// Null when the PR's fork repository was deleted.
    #[serde(deserialize_with = "required_nullable")]
    pub head_repository: Option<HeadRepository>,
    #[serde(deserialize_with = "required_nullable")]
    pub stack: Option<StackPage>,
    #[serde(flatten)]
    pub pr: StackPullRequest,
    pub base_ref_name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HeadRepository {
    pub owner: RepositoryOwner,
}

#[derive(Debug, Deserialize)]
pub(super) struct RepositoryOwner {
    pub login: String,
}

// deserialize_with makes the field required while still accepting explicit null.
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

#[derive(Debug, Deserialize)]
pub(super) struct NodeData {
    pub node: Option<StackPage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackPage {
    pub id: String,
    pub base_ref_name: String,
    pub size: usize,
    pub entries: Entries,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Entries {
    pub total_count: usize,
    pub page_info: PageInfo,
    pub nodes: Vec<Option<StackEntry>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackEntry {
    pub position: u32,
    #[serde(deserialize_with = "required_nullable")]
    pub pull_request: Option<StackPullRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackPullRequest {
    pub number: u32,
    pub url: String,
    pub head_ref_name: String,
    pub state: String,
    pub is_in_merge_queue: bool,
}
