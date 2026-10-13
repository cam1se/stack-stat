//! Git process adapter for worktree and branch-ref state.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use tokio::process::Command;

use crate::model::LocalSync;

use super::GitProvider;

/// Marker returned by GitProvider when HEAD is detached.
pub const DETACHED_HEAD: &str = "HEAD (detached)";

/// Git adapter bound to the discovered worktree and its shared common directory.
#[derive(Debug, Clone)]
pub struct CliGitProvider {
    worktree_root: PathBuf,
    common_dir: PathBuf,
}

impl CliGitProvider {
    /// Discover a worktree root and the repository's common Git directory.
    pub fn discover(cwd: &Path) -> Result<Self> {
        let root = Self::run_sync(cwd, &["rev-parse", "--show-toplevel"])
            .context("not in a git repository")?;
        let worktree_root = PathBuf::from(root.trim());
        let common = Self::run_sync(&worktree_root, &["rev-parse", "--git-common-dir"])?;
        let common_path = PathBuf::from(common.trim());
        let common_dir = if common_path.is_absolute() {
            common_path
        } else {
            worktree_root.join(common_path)
        }
        .canonicalize()
        .context("resolving git common directory")?;
        Ok(Self {
            worktree_root,
            common_dir,
        })
    }

    /// The checkout root whose `.stack-stat.toml` applies to this provider.
    #[must_use]
    pub fn worktree_root(&self) -> &Path {
        &self.worktree_root
    }

    /// Resolve a git revision expression to a complete commit SHA.
    pub async fn resolve_rev(&self, rev: &str) -> Result<String> {
        if rev.starts_with('-') {
            bail!("invalid revision: {rev}");
        }
        let expression = format!("{rev}^{{commit}}");
        self.run(&["rev-parse", "--verify", &expression]).await
    }

    fn run_sync(cwd: &Path, args: &[&str]) -> Result<String> {
        let output = StdCommand::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("running git {}", args.join(" ")))?;
        if !output.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout)
            .context("git output is not UTF-8")
            .map(|text| text.trim().to_owned())
    }

    async fn run(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.worktree_root)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("running git {}", args.join(" ")))?;
        if !output.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout)
            .context("git output is not UTF-8")
            .map(|text| text.trim().to_owned())
    }
}

#[async_trait]
impl GitProvider for CliGitProvider {
    async fn current_branch(&self) -> Result<String> {
        match self.run(&["symbolic-ref", "--short", "HEAD"]).await {
            Ok(branch) => Ok(branch),
            Err(error) => {
                let detached = self.run(&["rev-parse", "--verify", "HEAD"]).await.is_ok();
                if detached {
                    Ok(DETACHED_HEAD.into())
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn branch_shas(&self) -> Result<HashMap<String, String>> {
        let output = self
            .run(&[
                "for-each-ref",
                "--format=%(refname:short)%00%(objectname)",
                "refs/heads/",
            ])
            .await?;
        output
            .lines()
            .map(|line| {
                let (name, sha) = line
                    .split_once('\0')
                    .ok_or_else(|| anyhow!("git for-each-ref returned an invalid row"))?;
                Ok((name.into(), sha.into()))
            })
            .collect()
    }

    async fn current_sync(&self, branch: &str) -> Result<LocalSync> {
        if branch == DETACHED_HEAD {
            return Ok(LocalSync::InSync);
        }
        let upstream = match self
            .run(&[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ])
            .await
        {
            Ok(upstream) => upstream,
            Err(_) => {
                let fallback = format!("origin/{branch}");
                if self.resolve_rev(&fallback).await.is_err() {
                    return Ok(LocalSync::Unpushed);
                }
                fallback
            }
        };
        let range = format!("HEAD...{upstream}");
        let output = self
            .run(&["rev-list", "--left-right", "--count", &range])
            .await?;
        let mut counts = output.split_whitespace();
        let ahead = counts
            .next()
            .context("git rev-list did not report local-only commits")?
            .parse::<u64>()
            .context("git rev-list reported an invalid local count")?;
        let behind = counts
            .next()
            .context("git rev-list did not report remote-only commits")?
            .parse::<u64>()
            .context("git rev-list reported an invalid remote count")?;
        if counts.next().is_some() {
            bail!("git rev-list returned too many counts");
        }
        Ok(match (ahead > 0, behind > 0) {
            (false, false) => LocalSync::InSync,
            (true, false) => LocalSync::Ahead,
            (false, true) => LocalSync::Behind,
            (true, true) => LocalSync::Diverged,
        })
    }

    fn common_dir(&self) -> &Path {
        &self.common_dir
    }
}
