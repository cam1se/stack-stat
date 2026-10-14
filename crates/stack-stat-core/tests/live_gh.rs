//! End-to-end checks against the real `gh` and `gh stack` binaries and the live
//! fixture described in `docs/architecture/07-live-validation.md`.
//!
//! Skipped unless `STACK_STAT_LIVE=1`. Run with `make live-test`.

use stack_stat_core::{
    model::StackMode,
    providers::{
        CliGitHubProvider, GitHubError, GitHubProvider, StackBranch, StackViewResponse,
        graphql::classify_gh_failure,
    },
};
use std::{path::Path, process::Command};
use tempfile::TempDir;
use tokio::sync::Mutex;

/// Repository holding the fixture stacks.
fn fixture_repo() -> String {
    std::env::var("STACK_STAT_FIXTURE_REPO").unwrap_or_else(|_| "cam1se/stack-stat".into())
}

/// Fork of the fixture repository that holds the fork PR.
fn fork_repo() -> String {
    std::env::var("STACK_STAT_FIXTURE_FORK").unwrap_or_else(|_| "tcamise-gpsw/stack-stat".into())
}

/// Mutating tests share one remote fixture, so they take turns.
static MUTATION_LOCK: Mutex<()> = Mutex::const_new(());

macro_rules! live {
    () => {
        if std::env::var("STACK_STAT_LIVE").as_deref() != Ok("1") {
            eprintln!("skipped: set STACK_STAT_LIVE=1 to run live gh tests");
            return;
        }
    };
}

fn script(arg: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new("bash")
        .arg(root.join("scripts/live-fixture.sh"))
        .arg(arg)
        .status()
        .unwrap();
    assert!(status.success(), "live-fixture.sh {arg} failed");
}

/// Clone `slug` with `gh` and check out `branch`.
fn clone(slug: &str, branch: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new("gh")
        .args(["repo", "clone", slug])
        .arg(dir.path())
        .args(["--", "--branch", branch])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "clone {slug}@{branch}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    dir
}

fn provider(dir: &Path) -> CliGitHubProvider {
    let (owner, repo) = fixture_repo()
        .split_once('/')
        .map(|(o, r)| (o.to_owned(), r.to_owned()))
        .unwrap();
    CliGitHubProvider::new(dir.to_path_buf(), owner, repo)
}

/// Run `gh` in an empty, isolated config and data directory with a dummy token.
fn isolated_gh(args: &[&str], token: &str) -> (i32, String, String) {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new("gh")
        .args(args)
        .current_dir(home.path())
        .env("GH_TOKEN", token)
        .env("GH_CONFIG_DIR", home.path().join("config"))
        .env("XDG_DATA_HOME", home.path().join("data"))
        .env("XDG_CONFIG_HOME", home.path().join("xdg"))
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[tokio::test]
async fn resolves_owner_and_name_with_gh_repo_view() {
    live!();
    let dir = clone(&fixture_repo(), "fixture/s1-b");
    let resolved = CliGitHubProvider::resolve_repo(dir.path()).await.unwrap();
    let expected = fixture_repo();
    assert_eq!(format!("{}/{}", resolved.0, resolved.1), expected);
}

#[tokio::test]
async fn discovers_a_three_pr_stack_and_loads_details() {
    live!();
    let dir = clone(&fixture_repo(), "fixture/s1-b");
    let github = provider(dir.path());
    let stack = github.stack_view("fixture/s1-b").await.unwrap();
    assert_eq!(stack.mode, StackMode::Stacked);
    assert_eq!(stack.trunk, "main");
    let names: Vec<_> = stack.branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["fixture/s1-a", "fixture/s1-b", "fixture/s1-c"]);
    let numbers: Vec<_> = stack
        .branches
        .iter()
        .map(|b| b.pr.as_ref().unwrap().number)
        .collect();
    assert_eq!(numbers, [27, 28, 29]);
    assert!(stack.branches.iter().all(|b| !b.is_merged));
    let details = github.pr_details(&stack).await.unwrap();
    assert_eq!(details.prs.len(), 3);
    assert!(details.prs.iter().all(|pr| pr.is_draft));
    assert!(details.remote_rebase_branches.is_empty());
}

/// Assert a one-row single-mode view and return its row.
fn single_row(stack: &StackViewResponse) -> &StackBranch {
    assert_eq!(stack.mode, StackMode::Single);
    assert_eq!(stack.branches.len(), 1);
    &stack.branches[0]
}

#[tokio::test]
async fn branch_without_a_pull_request_is_a_single_row_on_the_default_branch() {
    live!();
    let dir = clone(&fixture_repo(), "fixture/s6-nopr");
    let github = provider(dir.path());
    let stack = github.stack_view("fixture/s6-nopr").await.unwrap();
    assert_eq!(stack.trunk, "main");
    let row = single_row(&stack);
    assert_eq!(row.name, "fixture/s6-nopr");
    assert!(row.pr.is_none());
    assert!(github.pr_details(&stack).await.unwrap().prs.is_empty());
    let unknown = github.stack_view("fixture/no-such-branch").await.unwrap();
    assert!(single_row(&unknown).pr.is_none());
}

#[tokio::test]
async fn unstacked_pull_request_is_a_single_row_based_on_its_base_ref() {
    live!();
    let dir = clone(&fixture_repo(), "fixture/s4-same");
    let stack = provider(dir.path())
        .stack_view("fixture/s4-same")
        .await
        .unwrap();
    assert_eq!(stack.trunk, "main");
    let pr = single_row(&stack).pr.as_ref().unwrap();
    assert_eq!((pr.number, pr.state.as_str()), (37, "OPEN"));
}

#[tokio::test]
async fn single_pull_request_uses_a_non_default_base_as_trunk() {
    live!();
    let dir = clone(&fixture_repo(), "fixture/s5-single");
    let github = provider(dir.path());
    let stack = github.stack_view("fixture/s5-single").await.unwrap();
    assert_eq!(stack.trunk, "fixture/single-base");
    let pr = single_row(&stack).pr.as_ref().unwrap();
    assert_eq!((pr.number, pr.state.as_str()), (43, "OPEN"));
    let details = github.pr_details(&stack).await.unwrap();
    assert_eq!(details.prs.len(), 1);
    assert!(details.prs[0].is_draft);
}

#[tokio::test]
async fn closed_and_merged_single_pull_requests_keep_their_lifecycle() {
    live!();
    let dir = clone(&fixture_repo(), "main");
    let github = provider(dir.path());
    let closed = github.stack_view("fixture/s7-closed").await.unwrap();
    let row = single_row(&closed);
    assert!(!row.is_merged);
    assert_eq!(row.pr.as_ref().map(|pr| pr.state.as_str()), Some("CLOSED"));
    let merged = github.stack_view("fixture/s8-merged").await.unwrap();
    let row = single_row(&merged);
    assert!(row.is_merged);
    assert_eq!(row.pr.as_ref().map(|pr| pr.state.as_str()), Some("MERGED"));
    assert_eq!(merged.trunk, "fixture/single-base");
}

#[tokio::test]
async fn merged_stack_keeps_deleted_branches() {
    live!();
    let dir = clone(&fixture_repo(), "main");
    let github = provider(dir.path());
    let stack = github.stack_view("fixture/s3-b").await.unwrap();
    assert_eq!(stack.mode, StackMode::Stacked);
    assert_eq!(stack.trunk, "fixture/trunk");
    let names: Vec<_> = stack.branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["fixture/s3-a", "fixture/s3-b"]);
    assert!(stack.branches.iter().all(|b| b.is_merged));
    let numbers: Vec<_> = stack
        .branches
        .iter()
        .map(|b| b.pr.as_ref().unwrap().number)
        .collect();
    assert_eq!(numbers, [34, 35]);
}

/// Two PRs use head branch `fixture/s1-c`: #29, the top of stack s1, and fork
/// PR #40, which is not stacked. The push remote decides which one is ours.
#[tokio::test]
async fn same_branch_name_resolves_by_head_repository_owner() {
    live!();
    let upstream = clone(&fixture_repo(), "fixture/s1-c");
    let stack = provider(upstream.path())
        .stack_view("fixture/s1-c")
        .await
        .unwrap();
    assert_eq!(stack.mode, StackMode::Stacked);
    assert_eq!(stack.branches.len(), 3);
    let fork = clone(&fork_repo(), "fixture/s1-c");
    let from_fork = provider(fork.path())
        .stack_view("fixture/s1-c")
        .await
        .unwrap();
    let pr = single_row(&from_fork).pr.as_ref().unwrap();
    assert_eq!(pr.number, 40);
}

#[tokio::test]
async fn push_without_local_tracking_is_classified() {
    live!();
    let dir = clone(&fixture_repo(), "fixture/s1-b");
    let error = provider(dir.path()).push_stack().await.unwrap_err();
    assert_eq!(error, GitHubError::LocalTrackingMissing);
}

#[tokio::test]
async fn push_outside_a_repository_is_a_plain_command_error() {
    live!();
    let dir = tempfile::tempdir().unwrap();
    let error = provider(dir.path()).push_stack().await.unwrap_err();
    assert!(
        matches!(&error, GitHubError::Command { code: 2, stderr } if stderr.contains("not a git repository")),
        "{error:?}"
    );
}

#[test]
fn missing_auth_is_classified() {
    live!();
    let (code, _, stderr) = isolated_gh(&["api", "user"], "");
    assert_eq!(code, 4, "{stderr}");
    assert_eq!(
        classify_gh_failure(&["api", "user"], code, &stderr),
        GitHubError::Auth
    );
}

/// gh 2.101.0 installs a missing extension on first use and exits 0 without
/// running the subcommand; the provider retries once after seeing this marker.
#[test]
fn gh_installs_missing_extension_on_first_use() {
    live!();
    let token = Command::new("gh").args(["auth", "token"]).output().unwrap();
    let token = String::from_utf8(token.stdout).unwrap();
    let (code, stdout, stderr) = isolated_gh(&["stack", "view"], token.trim());
    assert_eq!(code, 0, "{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains("Successfully installed github/gh-stack"),
        "{stdout}{stderr}"
    );
}

#[tokio::test]
async fn link_push_sync_and_draft_toggle_round_trip() {
    live!();
    let _guard = MUTATION_LOCK.lock().await;
    script("reset");
    let dir = clone(&fixture_repo(), "fixture/s1-b");
    let github = provider(dir.path());
    github.checkout_branch("fixture/s1-b").await.unwrap();
    github.push_stack().await.unwrap();
    github.sync_stack().await.unwrap();
    github.toggle_draft(27, true).await.unwrap();
    let details = github
        .pr_details(&github.stack_view("fixture/s1-b").await.unwrap())
        .await
        .unwrap();
    assert!(
        !details
            .prs
            .iter()
            .find(|p| p.number == 27)
            .unwrap()
            .is_draft
    );
    github.toggle_draft(27, false).await.unwrap();
    script("reset");
}

#[tokio::test]
async fn sync_against_conflicting_trunk_reports_rebase_conflict() {
    live!();
    let _guard = MUTATION_LOCK.lock().await;
    script("reset");
    let dir = clone(&fixture_repo(), "fixture/s2-b");
    let github = provider(dir.path());
    github.checkout_branch("fixture/s2-b").await.unwrap();
    script("conflict");
    let result = github.sync_stack().await;
    script("reset");
    assert_eq!(result.unwrap_err(), GitHubError::RebaseConflict);
}

#[tokio::test]
async fn single_pull_request_draft_toggle_round_trip() {
    live!();
    let _guard = MUTATION_LOCK.lock().await;
    script("reset");
    let dir = clone(&fixture_repo(), "fixture/s5-single");
    let github = provider(dir.path());
    github.toggle_draft(43, true).await.unwrap();
    let stack = github.stack_view("fixture/s5-single").await.unwrap();
    assert!(!github.pr_details(&stack).await.unwrap().prs[0].is_draft);
    github.toggle_draft(43, false).await.unwrap();
    script("reset");
}
