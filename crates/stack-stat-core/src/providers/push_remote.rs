//! Owner of the repository a local branch pushes to, from Git configuration.

use std::{path::Path, process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};

const GIT_TIMEOUT: Duration = Duration::from_secs(5);

/// GitHub owner of the remote `branch` pushes to, or `None` when Git does not
/// say or the remote URL is not a recognizable `owner/repo` path.
///
/// Remote choice follows Git precedence: `branch.<name>.pushRemote`,
/// `remote.pushDefault`, `branch.<name>.remote`, then `origin`.
pub(super) async fn push_owner(worktree_root: &Path, branch: &str) -> Option<String> {
    let remote = remote_name(worktree_root, branch).await;
    let url = git(worktree_root, &["remote", "get-url", "--push", &remote]).await?;
    owner_from_url(&url)
}

async fn remote_name(worktree_root: &Path, branch: &str) -> String {
    for key in [
        format!("branch.{branch}.pushRemote"),
        "remote.pushDefault".to_owned(),
        format!("branch.{branch}.remote"),
    ] {
        if let Some(name) = git(worktree_root, &["config", "--get", &key]).await
            && !name.is_empty()
        {
            return name;
        }
    }
    "origin".to_owned()
}

async fn git(worktree_root: &Path, args: &[&str]) -> Option<String> {
    let output = timeout(
        GIT_TIMEOUT,
        Command::new("git")
            .args(args)
            .current_dir(worktree_root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

/// Owner segment of `https://host/owner/repo(.git)`, `ssh://git@host/owner/repo`
/// and scp-style `git@host:owner/repo(.git)` remote URLs.
pub(super) fn owner_from_url(url: &str) -> Option<String> {
    let path = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('/')?.1,
        None => url.split_once(':')?.1,
    };
    let mut segments = path.trim_matches('/').split('/');
    let owner = segments.next()?;
    // A repository segment must follow the owner.
    segments.next()?;
    (!owner.is_empty()).then(|| owner.to_owned())
}

#[cfg(test)]
mod tests {
    use super::owner_from_url;

    #[test]
    fn recognizes_common_github_remote_forms() {
        for url in [
            "https://github.com/alice/stack-stat.git",
            "https://github.com/alice/stack-stat",
            "https://user@github.com/alice/stack-stat/",
            "ssh://git@github.com/alice/stack-stat.git",
            "git@github.com:alice/stack-stat.git",
        ] {
            assert_eq!(owner_from_url(url).as_deref(), Some("alice"), "{url}");
        }
    }

    #[test]
    fn rejects_urls_without_owner_and_repository() {
        for url in ["", "origin", "/local/path", "https://github.com/alice"] {
            assert_eq!(owner_from_url(url), None, "{url}");
        }
    }
}
