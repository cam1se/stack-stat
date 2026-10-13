//! Non-interactive GitHub CLI adapter for stack and pull request state.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use async_trait::async_trait;
use tokio::{process::Command, time::timeout};

use super::{
    GitHubError, GitHubProvider, PrResponse, StackViewResponse,
    graphql::{build_pr_query, classify_gh_failure, parse_pr_response},
};

const READ_TIMEOUT: Duration = Duration::from_secs(20);
const MUTATION_TIMEOUT: Duration = Duration::from_secs(120);

/// `gh` adapter bound to one repository worktree and GitHub repository identity.
#[derive(Clone, Debug)]
pub struct CliGitHubProvider {
    worktree_root: PathBuf,
    owner: String,
    repo: String,
}

impl CliGitHubProvider {
    /// Bind an authenticated GitHub repository by its owner and name.
    #[must_use]
    pub fn new(worktree_root: PathBuf, owner: String, repo: String) -> Self {
        Self {
            worktree_root,
            owner,
            repo,
        }
    }

    /// Resolve owner and repository name once through `gh repo view` with a
    /// bounded, non-interactive read just like other GitHub queries.
    pub async fn resolve_repo(worktree_root: &Path) -> Result<(String, String), GitHubError> {
        let temporary = Self::new(worktree_root.to_path_buf(), String::new(), String::new());
        let raw = temporary
            .run(&["repo", "view", "--json", "owner,name"], READ_TIMEOUT)
            .await?;
        let data: serde_json::Value =
            serde_json::from_str(&raw).map_err(|error| GitHubError::Parse(error.to_string()))?;
        let owner = data
            .pointer("/owner/login")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| GitHubError::Parse("gh repo view omitted owner.login".into()))?;
        let name = data
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| GitHubError::Parse("gh repo view omitted name".into()))?;
        Ok((owner.into(), name.into()))
    }

    async fn run(&self, args: &[&str], deadline: Duration) -> Result<String, GitHubError> {
        self.run_output(args, deadline)
            .await
            .map(|(stdout, _)| stdout)
    }

    async fn run_output(
        &self,
        args: &[&str],
        deadline: Duration,
    ) -> Result<(String, String), GitHubError> {
        let output = self.run_process(args, deadline).await?;
        if !output.status.success() {
            return Err(classify_gh_failure(
                args,
                output.status.code().unwrap_or(1),
                &String::from_utf8_lossy(&output.stderr),
            ));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|error| GitHubError::Parse(error.to_string()))?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok((stdout, stderr))
    }

    async fn run_process(
        &self,
        args: &[&str],
        deadline: Duration,
    ) -> Result<std::process::Output, GitHubError> {
        timeout(
            deadline,
            Command::new("gh")
                .args(args)
                .current_dir(&self.worktree_root)
                .env("GH_PROMPT_DISABLED", "1")
                .env("NO_COLOR", "1")
                .env("GH_NO_UPDATE_NOTIFIER", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| GitHubError::Network("timed out".into()))?
        .map_err(|error| GitHubError::Network(error.to_string()))
    }
}

#[async_trait]
impl GitHubProvider for CliGitHubProvider {
    async fn stack_view(&self) -> Result<StackViewResponse, GitHubError> {
        let raw = self.run(&["stack", "view", "--json"], READ_TIMEOUT).await?;
        serde_json::from_str(&raw)
            .map_err(|error| GitHubError::Parse(format!("gh stack view: {error}")))
    }

    fn repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    async fn pr_details(&self, numbers: &[u32]) -> Result<Vec<PrResponse>, GitHubError> {
        if numbers.is_empty() {
            return Ok(Vec::new());
        }
        let query = build_pr_query(&self.owner, &self.repo, numbers);
        let input = format!("query={query}");
        let output = self
            .run_process(&["api", "graphql", "-f", &input], READ_TIMEOUT)
            .await?;
        let raw = String::from_utf8(output.stdout)
            .map_err(|error| GitHubError::Parse(error.to_string()))?;
        classify_graphql_result(
            output.status.code().unwrap_or(1),
            &raw,
            &String::from_utf8_lossy(&output.stderr),
        )
    }

    async fn open_pr_in_browser(&self, number: u32) -> Result<(), GitHubError> {
        self.run(&["pr", "view", &number.to_string(), "--web"], READ_TIMEOUT)
            .await
            .map(|_| ())
    }

    async fn checkout_branch(&self, branch: &str) -> Result<(), GitHubError> {
        self.run(&["stack", "checkout", branch], MUTATION_TIMEOUT)
            .await
            .map(|_| ())
    }

    async fn push_stack(&self) -> Result<(), GitHubError> {
        self.run(&["stack", "push"], MUTATION_TIMEOUT)
            .await
            .map(|_| ())
    }

    async fn sync_stack(&self) -> Result<String, GitHubError> {
        let (stdout, stderr) = self
            .run_output(&["stack", "sync"], MUTATION_TIMEOUT)
            .await?;
        classify_sync_output(&stdout, &stderr)
    }

    async fn toggle_draft(&self, number: u32, make_ready: bool) -> Result<(), GitHubError> {
        let number = number.to_string();
        let args: &[&str] = if make_ready {
            &["pr", "ready", &number]
        } else {
            &["pr", "ready", &number, "--undo"]
        };
        self.run(args, MUTATION_TIMEOUT).await.map(|_| ())
    }
}

/// Distinguish an exit-0 divergent sync from an actual completed sync.
pub fn classify_sync_output(stdout: &str, stderr: &str) -> Result<String, GitHubError> {
    if stdout.to_ascii_lowercase().contains("sync aborted")
        || stderr.to_ascii_lowercase().contains("sync aborted")
    {
        return Err(GitHubError::Diverged(
            "local and remote stacks diverged".into(),
        ));
    }
    let mut combined = stdout.trim().to_owned();
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr.trim());
    }
    Ok(combined)
}

/// Preserve GraphQL's partial repository data even when `gh api` exits 1.
pub fn classify_graphql_result(
    code: i32,
    stdout: &str,
    stderr: &str,
) -> Result<Vec<PrResponse>, GitHubError> {
    let parsed = parse_pr_response(stdout);
    if parsed.is_ok() || code == 0 {
        return parsed;
    }
    let error = classify_gh_failure(&["api", "graphql"], code, stderr);
    if matches!(error, GitHubError::Command { .. }) && !stdout.trim().is_empty() {
        parsed
    } else {
        Err(error)
    }
}
