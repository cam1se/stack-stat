//! Normalize an ordinary PR or a branch awaiting its first PR.

use crate::{
    model::StackMode,
    providers::{GitHubError, StackBranch, StackPr, StackViewResponse},
};

use super::wire::{Candidate, DefaultBranch};

pub(super) fn from_candidate(
    current_branch: &str,
    candidate: Candidate,
) -> Result<StackViewResponse, GitHubError> {
    let pr = candidate.pr;
    if pr.number == 0
        || pr.url.trim().is_empty()
        || pr.head_ref_name != current_branch
        || candidate.base_ref_name.trim().is_empty()
        || !matches!(pr.state.as_str(), "OPEN" | "MERGED" | "CLOSED")
    {
        return Err(GitHubError::Parse(
            "GraphQL single PR metadata is invalid".into(),
        ));
    }
    Ok(view(
        current_branch,
        candidate.base_ref_name,
        StackBranch {
            name: pr.head_ref_name,
            is_merged: pr.state == "MERGED",
            is_queued: pr.is_in_merge_queue,
            pr: Some(StackPr {
                number: pr.number,
                url: pr.url,
                state: pr.state,
            }),
        },
    ))
}

pub(super) fn without_pr(
    current_branch: &str,
    default_branch: Option<DefaultBranch>,
) -> Result<StackViewResponse, GitHubError> {
    let trunk = default_branch
        .map(|branch| branch.name)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            GitHubError::Parse("GraphQL branch discovery had no default branch name".into())
        })?;
    Ok(view(
        current_branch,
        trunk,
        StackBranch {
            name: current_branch.to_owned(),
            is_merged: false,
            is_queued: false,
            pr: None,
        },
    ))
}

fn view(current_branch: &str, trunk: String, branch: StackBranch) -> StackViewResponse {
    StackViewResponse {
        mode: StackMode::Single,
        trunk,
        current_branch: current_branch.to_owned(),
        branches: vec![branch],
    }
}
