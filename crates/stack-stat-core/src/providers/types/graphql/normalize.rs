//! Normalization of GraphQL wire records into provider responses.

use std::collections::BTreeSet;

use super::super::{
    PrCheckResponse, PrResponse, ReviewRequest, ReviewResponse, StackDetailsResponse,
    StackViewResponse,
};
use super::{GraphqlResponse, RawCheck, RawPr, RawRepositoryAlias};

impl GraphqlResponse {
    /// Normalize available PR details and remote rebase comparisons for a stack.
    #[must_use]
    pub fn into_stack_details(self, stack: &StackViewResponse) -> StackDetailsResponse {
        let Some(repository) = self.data.and_then(|data| data.repository) else {
            return StackDetailsResponse::default();
        };
        let mut aliases = repository.aliases;
        let mut remote_rebase_branches = BTreeSet::new();
        for (index, branch) in stack
            .branches
            .iter()
            .filter(|branch| !branch.is_merged)
            .enumerate()
        {
            let Some(Some(RawRepositoryAlias::Ref(reference))) =
                aliases.remove(&format!("rebase{index}"))
            else {
                continue;
            };
            if reference
                .compare
                .and_then(|comparison| comparison.behind_by)
                .is_some_and(|behind_by| behind_by > 0)
            {
                remote_rebase_branches.insert(branch.name.clone());
            }
        }
        let prs = aliases
            .into_values()
            .flatten()
            .filter_map(|alias| match alias {
                RawRepositoryAlias::Pr(pr) => Some(pr.normalize()),
                RawRepositoryAlias::Ref(_) => None,
            })
            .collect();
        StackDetailsResponse {
            prs,
            remote_rebase_branches,
        }
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
