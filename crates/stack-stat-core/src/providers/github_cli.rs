//! Non-interactive GitHub CLI adapter for stack and pull request state.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use async_trait::async_trait;
use tokio::{process::Command, time::timeout};

use super::{
    GitHubError, GitHubProvider, StackDetailsResponse, StackViewResponse,
    graphql::{StackDiscovery, build_stack_query, classify_gh_failure, parse_stack_response},
    push_remote,
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
        let mut output = self.run_process(args, deadline).await?;
        if args.first() == Some(&"stack") && output.status.success() && installed_extension(&output)
        {
            // On first use gh installs the extension and exits 0 without running the
            // subcommand, so the requested action has not happened yet.
            output = self.run_process(args, deadline).await?;
        }
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

/// Whether `gh` auto-installed the stack extension instead of running the command.
fn installed_extension(output: &std::process::Output) -> bool {
    const MARKER: &str = "Successfully installed github/gh-stack";
    String::from_utf8_lossy(&output.stdout).contains(MARKER)
        || String::from_utf8_lossy(&output.stderr).contains(MARKER)
}

#[async_trait]
impl GitHubProvider for CliGitHubProvider {
    async fn stack_view(&self, current_branch: &str) -> Result<StackViewResponse, GitHubError> {
        let discovery = StackDiscovery::new(current_branch)?;
        let head_owner = push_remote::push_owner(&self.worktree_root, current_branch)
            .await
            .unwrap_or_else(|| self.owner.clone());
        let mut discovery = discovery.with_head_owner(head_owner);
        loop {
            let input = format!("query={}", discovery.query(&self.owner, &self.repo));
            let output = self
                .run_process(&["api", "graphql", "-f", &input], READ_TIMEOUT)
                .await?;
            let raw = String::from_utf8(output.stdout)
                .map_err(|error| GitHubError::Parse(error.to_string()))?;
            if let Some(stack) = discovery.accept_response(
                output.status.code().unwrap_or(1),
                &raw,
                &String::from_utf8_lossy(&output.stderr),
            )? {
                return Ok(stack);
            }
        }
    }

    fn repo(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    async fn pr_details(
        &self,
        stack: &StackViewResponse,
    ) -> Result<StackDetailsResponse, GitHubError> {
        if !stack.branches.iter().any(|branch| branch.pr.is_some()) {
            return Ok(StackDetailsResponse::default());
        }
        let query = build_stack_query(&self.owner, &self.repo, stack);
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
            stack,
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

    async fn push_branch(&self) -> Result<(), GitHubError> {
        let output = timeout(
            MUTATION_TIMEOUT,
            Command::new("git")
                .arg("push")
                .current_dir(&self.worktree_root)
                .env("GIT_TERMINAL_PROMPT", "0")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| GitHubError::Git("git push timed out".into()))?
        .map_err(|error| GitHubError::Git(error.to_string()))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(GitHubError::Git(push_failure_message(
                &String::from_utf8_lossy(&output.stderr),
            )))
        }
    }

    async fn sync_stack(&self) -> Result<String, GitHubError> {
        let (stdout, stderr) = self
            .run_output(&["stack", "sync"], MUTATION_TIMEOUT)
            .await?;
        classify_sync_output(&stdout, &stderr)
    }

    async fn rebase_stack(&self) -> Result<String, GitHubError> {
        let (stdout, stderr) = self
            .run_output(&["stack", "rebase"], MUTATION_TIMEOUT)
            .await?;
        Ok([stdout.trim(), stderr.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n"))
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

/// Lead a failed push's stderr with its cause; only the first line reaches the status bar.
///
/// Git prints the remote (`To <url>`) and hook output before a rejection, so the first
/// `[rejected]`, `fatal:`, or `error:` line moves to the front. The full stderr follows it.
fn push_failure_message(stderr: &str) -> String {
    let stderr = stderr.trim();
    let lines = || stderr.lines().map(str::trim);
    let headline = lines()
        .find(|line| line.contains("rejected]"))
        .map(|line| {
            line.trim_start_matches('!')
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .or_else(|| {
            lines()
                .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
                .map(str::to_owned)
        });
    match headline {
        Some(headline) if lines().next() != Some(headline.as_str()) => {
            format!("{headline}\n{stderr}")
        }
        _ => stderr.to_owned(),
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

/// Preserve partial stack details when `gh api` exits 1.
pub fn classify_graphql_result(
    code: i32,
    stdout: &str,
    stderr: &str,
    stack: &StackViewResponse,
) -> Result<StackDetailsResponse, GitHubError> {
    let parsed = parse_stack_response(stdout, stack);
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
