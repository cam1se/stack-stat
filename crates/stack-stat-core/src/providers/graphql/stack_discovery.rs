//! Branch-to-stack selection and complete, position-ordered entry pagination.

use crate::providers::{DETACHED_HEAD, GitHubError, StackBranch, StackPr, StackViewResponse};

mod wire;
use wire::{LookupData, NodeData, StackPage, parse_response};

/// Incremental GraphQL reader for the GitHub stack containing a Git branch.
///
/// Send [`Self::query`] through `gh api graphql`, then pass the result to
/// [`Self::accept_response`]. `None` requests another stack-entry page; `Some`
/// contains the complete stack. Discard the reader after any error. Reading a
/// stack never depends on the gh-stack extension or its local tracking.
#[derive(Debug)]
pub struct StackDiscovery<'a> {
    current_branch: &'a str,
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
            stack: None,
            entries: Vec::new(),
            cursor: None,
        })
    }

    /// Build the initial branch lookup or the next stack-entry page query.
    ///
    /// The initial request selects the newest open PR and the newest PR across
    /// all lifecycle states, both ordered by creation time. Two `first:1`
    /// connections preserve open-PR preference without truncating branch history.
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
            "query {{ repository(owner:{owner},name:{repo}) {{ \
             openPullRequests: pullRequests(first:1,headRefName:{branch},states:[OPEN],{order}) \
             {{ nodes {{ stack {{ {fields} }} }} }} \
             pullRequests(first:1,headRefName:{branch},states:[OPEN,MERGED,CLOSED],{order}) \
             {{ nodes {{ stack {{ {fields} }} }} }} }} }}"
        )
    }

    /// Consume one `gh api graphql` result, returning only a complete stack.
    ///
    /// Prefer the newest open PR even when its stack is null; only an absent
    /// open PR permits the newest lifecycle PR fallback. No PR or an explicitly
    /// null stack returns `NotInStack`. Failed queries, inaccessible entries,
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
            let candidate = repository
                .open_pull_requests
                .nodes
                .into_iter()
                .next()
                .or_else(|| repository.pull_requests.nodes.into_iter().next())
                .ok_or(GitHubError::NotInStack)?
                .ok_or_else(|| GitHubError::Parse("GraphQL branch lookup had a null PR".into()))?;
            candidate.stack.ok_or(GitHubError::NotInStack)?
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
            trunk: stack.trunk,
            current_branch: self.current_branch.to_owned(),
            branches: std::mem::take(&mut self.entries)
                .into_iter()
                .map(|(_, branch)| branch)
                .collect(),
        }))
    }
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
