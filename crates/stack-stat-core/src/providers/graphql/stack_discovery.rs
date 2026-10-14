//! Branch-to-stack or single-mode selection with strict stack pagination.

use crate::{
    model::StackMode,
    providers::{DETACHED_HEAD, GitHubError, StackBranch, StackPr, StackViewResponse},
};

mod single;
mod wire;
use wire::{Candidate, LookupData, NodeData, StackPage, parse_response};

/// Incremental GraphQL reader for a Git branch's stack or single-mode view.
///
/// Send [`Self::query`] through `gh api graphql`, then pass the result to
/// [`Self::accept_response`]. `None` requests another stack-entry page; `Some`
/// contains the complete view. Discard the reader after any error. Discovery
/// never depends on the gh-stack extension or its local tracking.
#[derive(Debug)]
pub struct StackDiscovery<'a> {
    current_branch: &'a str,
    head_owner: Option<String>,
    stack: Option<StackIdentity>,
    entries: Vec<(u32, StackBranch)>,
    cursor: Option<String>,
}

#[derive(Debug)]
struct StackIdentity {
    id: String,
    trunk: String,
    size: usize,
}

impl<'a> StackDiscovery<'a> {
    /// Start discovery for the checked-out branch supplied by Git.
    ///
    /// An empty branch or [`DETACHED_HEAD`] returns `NotInStack` without a query.
    pub fn new(current_branch: &'a str) -> Result<Self, GitHubError> {
        if current_branch.is_empty() || current_branch == DETACHED_HEAD {
            return Err(GitHubError::NotInStack);
        }
        Ok(Self {
            current_branch,
            head_owner: None,
            stack: None,
            entries: Vec::new(),
            cursor: None,
        })
    }

    /// Accept only PRs whose head repository is owned by `owner` (compared
    /// case-insensitively), so a fork PR that reuses the branch name is never
    /// mistaken for this checkout's PR. Without this, any head repository
    /// matches.
    #[must_use]
    pub fn with_head_owner(mut self, owner: impl Into<String>) -> Self {
        self.head_owner = Some(owner.into());
        self
    }

    /// Build the initial branch lookup or the next stack-entry page query.
    ///
    /// The initial request selects the newest open PR and the newest PR across
    /// all lifecycle states, both ordered by creation time. Each connection
    /// returns a few candidates so PRs from other head repositories can be
    /// skipped; open-PR preference is kept without truncating branch history.
    /// Continuations use the selected stack's node ID and entry cursor.
    #[must_use]
    pub fn query(&self, owner: &str, repo: &str) -> String {
        let fields = stack_fields(self.cursor.as_deref());
        if let Some(stack) = &self.stack {
            let id = quoted(&stack.id);
            return format!(
                "query {{ node(id:{id}) {{ ... on PullRequestStack {{ {fields} }} }} }}"
            );
        }
        let owner = quoted(owner);
        let repo = quoted(repo);
        let branch = quoted(self.current_branch);
        let order = "orderBy:{field:CREATED_AT,direction:DESC}";
        format!(
            "query {{ repository(owner:{owner},name:{repo}) {{ defaultBranchRef {{ name }} \
             openPullRequests: pullRequests(first:{CANDIDATES},headRefName:{branch},states:[OPEN],{order}) \
             {{ nodes {{ number url headRefName baseRefName state isInMergeQueue \
             headRepository {{ owner {{ login }} }} stack {{ {fields} }} }} }} \
             pullRequests(first:{CANDIDATES},headRefName:{branch},states:[OPEN,MERGED,CLOSED],{order}) \
             {{ nodes {{ number url headRefName baseRefName state isInMergeQueue \
             headRepository {{ owner {{ login }} }} stack {{ {fields} }} }} }} }} }}"
        )
    }

    /// Consume one `gh api graphql` result, returning only a complete view.
    ///
    /// Prefer the newest open PR even when its stack is null; only an absent
    /// open PR permits the newest lifecycle PR fallback. An explicitly null
    /// stack becomes a single-PR view; no PR becomes a current-branch row using
    /// the repository default branch. Failed queries, inaccessible entries,
    /// malformed metadata, and truncated pagination remain errors. Unlike PR
    /// details and rebase comparisons, discovery cannot use incomplete data.
    pub fn accept_response(
        &mut self,
        code: i32,
        stdout: &str,
        stderr: &str,
    ) -> Result<Option<StackViewResponse>, GitHubError> {
        let page = if self.stack.is_some() {
            let data: NodeData = parse_response(code, stdout, stderr)?;
            data.node.ok_or_else(|| {
                GitHubError::Parse("GraphQL stack continuation had no stack node".into())
            })?
        } else {
            let data: LookupData = parse_response(code, stdout, stderr)?;
            let repository = data.repository.ok_or_else(|| {
                GitHubError::Parse("GraphQL stack discovery had no repository data".into())
            })?;
            let head_owner = self.head_owner.as_deref();
            let candidate = select_candidate(repository.open_pull_requests.nodes, head_owner)?
                .map_or_else(
                    || select_candidate(repository.pull_requests.nodes, head_owner),
                    |candidate| Ok(Some(candidate)),
                )?;
            match candidate {
                Some(Candidate {
                    stack: Some(page), ..
                }) => page,
                Some(candidate) => {
                    return single::from_candidate(self.current_branch, candidate).map(Some);
                }
                None => {
                    return single::without_pr(self.current_branch, repository.default_branch_ref)
                        .map(Some);
                }
            }
        };
        self.accept_page(page)
    }

    fn accept_page(&mut self, page: StackPage) -> Result<Option<StackViewResponse>, GitHubError> {
        if page.size == 0 || page.entries.total_count != page.size {
            return Err(GitHubError::Parse(
                "GitHub stack entry count does not match its size".into(),
            ));
        }
        if let Some(stack) = &self.stack {
            if stack.id != page.id || stack.trunk != page.base_ref_name || stack.size != page.size {
                return Err(GitHubError::Parse(
                    "GitHub stack changed during pagination".into(),
                ));
            }
        } else {
            self.entries = Vec::with_capacity(page.size);
            self.stack = Some(StackIdentity {
                id: page.id,
                trunk: page.base_ref_name,
                size: page.size,
            });
        }
        let has_next_page = page.entries.page_info.has_next_page;
        let next_cursor = if has_next_page {
            let cursor = page
                .entries
                .page_info
                .end_cursor
                .filter(|cursor| !cursor.is_empty() && self.cursor.as_ref() != Some(cursor))
                .ok_or_else(|| {
                    GitHubError::Parse("GitHub stack pagination cursor did not advance".into())
                })?;
            if page.entries.nodes.is_empty() {
                return Err(GitHubError::Parse(
                    "GitHub stack pagination returned an empty page".into(),
                ));
            }
            Some(cursor)
        } else {
            None
        };
        for entry in page.entries.nodes {
            let entry = entry.ok_or_else(|| {
                GitHubError::Parse("GitHub stack contains an inaccessible entry".into())
            })?;
            let pr = entry.pull_request.ok_or_else(|| {
                GitHubError::Parse("GitHub stack contains an inaccessible pull request".into())
            })?;
            self.entries.push((
                entry.position,
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
            ));
        }
        if self.entries.len() > page.size
            || (!has_next_page && self.entries.len() != page.size)
            || (has_next_page && self.entries.len() == page.size)
        {
            return Err(GitHubError::Parse(
                "GitHub stack pagination returned an incomplete entry set".into(),
            ));
        }
        self.cursor = next_cursor;
        if has_next_page {
            return Ok(None);
        }
        self.entries.sort_unstable_by_key(|(position, _)| *position);
        if self.entries[0].0 == 0 || self.entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(GitHubError::Parse(
                "GitHub stack entry positions are invalid or duplicated".into(),
            ));
        }
        let stack = self
            .stack
            .take()
            .expect("stack metadata was recorded before its entries");
        Ok(Some(StackViewResponse {
            mode: StackMode::Stacked,
            trunk: stack.trunk,
            current_branch: self.current_branch.to_owned(),
            branches: std::mem::take(&mut self.entries)
                .into_iter()
                .map(|(_, branch)| branch)
                .collect(),
        }))
    }
}

/// Maximum PRs fetched per connection when skipping foreign head repositories.
const CANDIDATES: u8 = 10;

/// First candidate whose head repository matches, or `None` when none does.
/// A null node is malformed data rather than absence.
fn select_candidate(
    nodes: Vec<Option<Candidate>>,
    head_owner: Option<&str>,
) -> Result<Option<Candidate>, GitHubError> {
    for node in nodes {
        let candidate =
            node.ok_or_else(|| GitHubError::Parse("GraphQL branch lookup had a null PR".into()))?;
        let matches = head_owner.is_none_or(|owner| {
            candidate
                .head_repository
                .as_ref()
                .is_some_and(|repository| repository.owner.login.eq_ignore_ascii_case(owner))
        });
        if matches {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn stack_fields(cursor: Option<&str>) -> String {
    let after = cursor.map_or_else(String::new, |cursor| format!(",after:{}", quoted(cursor)));
    format!(
        "id baseRefName size entries(first:100{after}) {{ totalCount \
         pageInfo {{ hasNextPage endCursor }} \
         nodes {{ position pullRequest {{ number url headRefName state isInMergeQueue }} }} }}"
    )
}
