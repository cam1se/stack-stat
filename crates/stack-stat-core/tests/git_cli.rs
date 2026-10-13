use std::{fs, path::Path, process::Command};

use stack_stat_core::{
    model::LocalSync,
    providers::{CliGitProvider, GitProvider},
};

fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn commit(cwd: &Path, message: &str) {
    git(
        cwd,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    );
}

async fn current_sync(git_provider: &CliGitProvider) -> LocalSync {
    let branch = git_provider.current_branch().await.unwrap();
    git_provider.current_sync(&branch).await.unwrap()
}

#[tokio::test]
async fn worktree_uses_shared_common_dir_and_its_own_head() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let other = temp.path().join("other");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "first",
        ],
    );
    let first = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "second",
        ],
    );
    git(&repo, &["branch", "feat/api"]);
    git(
        &repo,
        &["worktree", "add", other.to_str().unwrap(), "feat/api"],
    );

    let main = CliGitProvider::discover(&repo).unwrap();
    let feature = CliGitProvider::discover(&other).unwrap();
    assert_eq!(main.common_dir(), feature.common_dir());
    assert_eq!(
        feature.worktree_root().canonicalize().unwrap(),
        other.canonicalize().unwrap()
    );
    assert_eq!(main.current_branch().await.unwrap(), "main");
    assert_eq!(feature.current_branch().await.unwrap(), "feat/api");
    assert_eq!(feature.branch_shas().await.unwrap().len(), 2);
    assert_eq!(feature.resolve_rev("HEAD~1").await.unwrap(), first);
    assert_eq!(feature.resolve_rev(&first[..8]).await.unwrap(), first);

    git(&other, &["checkout", "--detach", "HEAD"]);
    assert_eq!(feature.current_branch().await.unwrap(), "HEAD (detached)");
}

#[test]
fn discover_fails_outside_repository() {
    let temp = tempfile::tempdir().unwrap();
    let err = CliGitProvider::discover(temp.path())
        .unwrap_err()
        .to_string();
    assert!(err.contains("git repository"), "{err}");
}

#[tokio::test]
async fn current_sync_classifies_local_branch_against_remote_tracking_ref() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let repo = temp.path().join("repo");
    let collaborator = temp.path().join("collaborator");
    fs::create_dir(&repo).unwrap();
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&repo, &["init", "-b", "main"]);
    commit(&repo, "base");
    let base = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-u", "origin", "main"]);

    let provider = CliGitProvider::discover(&repo).unwrap();
    assert_eq!(current_sync(&provider).await, LocalSync::InSync);

    commit(&repo, "ahead");
    assert_eq!(current_sync(&provider).await, LocalSync::Ahead);
    git(&repo, &["reset", "--hard", "origin/main"]);
    git(&repo, &["branch", "--unset-upstream", "main"]);
    assert_eq!(
        current_sync(&provider).await,
        LocalSync::InSync,
        "origin/<branch> is used when no upstream is configured"
    );

    git(
        temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            remote.to_str().unwrap(),
            collaborator.to_str().unwrap(),
        ],
    );
    commit(&collaborator, "behind");
    git(&collaborator, &["push", "origin", "main"]);
    git(&repo, &["fetch", "origin"]);
    assert_eq!(current_sync(&provider).await, LocalSync::Behind);

    git(&repo, &["reset", "--hard", &base]);
    commit(&repo, "diverged");
    assert_eq!(current_sync(&provider).await, LocalSync::Diverged);

    git(&repo, &["checkout", "-b", "unpublished"]);
    assert_eq!(current_sync(&provider).await, LocalSync::Unpushed);
    git(&repo, &["config", "branch.unpublished.remote", "origin"]);
    git(
        &repo,
        &[
            "config",
            "branch.unpublished.merge",
            "refs/heads/unpublished",
        ],
    );
    assert_eq!(current_sync(&provider).await, LocalSync::Unpushed);

    git(&repo, &["checkout", "--detach", "HEAD"]);
    assert_eq!(current_sync(&provider).await, LocalSync::InSync);
}
