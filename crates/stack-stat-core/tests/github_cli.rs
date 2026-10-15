use std::{collections::BTreeSet, fs, path::Path, process::Command};

use stack_stat_core::{
    model::StackMode,
    providers::{
        CliGitHubProvider, GitHubError, GitHubProvider, StackBranch, StackViewResponse,
        classify_graphql_result, classify_sync_output,
        graphql::{StackDiscovery, build_stack_query, classify_gh_failure, parse_stack_response},
    },
};

fn stack() -> StackViewResponse {
    StackDiscovery::new("api")
        .unwrap()
        .accept_response(0, include_str!("fixtures/stack_view.json"), "")
        .unwrap()
        .unwrap()
}

#[test]
fn query_is_sorted_deduplicated_and_compares_each_non_merged_layer() {
    let mut stack = stack();
    stack.branches[2].pr.as_mut().unwrap().number = 43;
    let query = build_stack_query("example", "project", &stack);
    let p42 = query.find("pr42: pullRequest(number: 42)").unwrap();
    let p43 = query.find("pr43: pullRequest(number: 43)").unwrap();
    assert!(p42 < p43);
    assert_eq!(query.matches("pr43: pullRequest").count(), 1);
    for field in [
        "reviewThreads(first:100)",
        "reviewRequests",
        "latestReviews",
        "statusCheckRollup",
        "StatusContext",
        "CheckRun",
        "__typename",
    ] {
        assert!(query.contains(field), "missing {field}: {query}");
    }
    assert!(query.contains(
        "rebase0: ref(qualifiedName:\"refs/heads/main\") { compare(headRef:\"api\") { behindBy } }"
    ));
    assert!(query.contains(
        "rebase1: ref(qualifiedName:\"refs/heads/api\") { compare(headRef:\"ui\") { behindBy } }"
    ));
    assert!(!query.contains("rebase2:"));
    assert!(!query.contains("needsRebase"));
}

#[test]
fn parser_keeps_partial_data_and_omits_inaccessible_pr() {
    let details = parse_stack_response(include_str!("fixtures/pr_partial.json"), &stack()).unwrap();
    assert_eq!(details.prs.len(), 1);
    assert_eq!(details.prs[0].number, 42);
    assert!(details.prs[0].checks.is_empty());
    assert_eq!(details.prs[0].review_decision, None);
    assert_eq!(details.prs[0].unresolved_threads, 0);
    assert_eq!(details.prs[0].review_requests[0].author, "reviewers");
}

#[test]
fn parser_marks_layer_for_positive_remote_behind_by() {
    let raw = include_str!("fixtures/pr_details.json").replacen(
        "\"repository\": {",
        "\"repository\": {\"rebase1\": {\"compare\": {\"behindBy\": 1}},",
        1,
    );

    let details = parse_stack_response(&raw, &stack()).unwrap();

    assert_eq!(
        details.prs.iter().map(|pr| pr.number).collect::<Vec<_>>(),
        [42, 43]
    );
    assert_eq!(
        details.remote_rebase_branches,
        BTreeSet::from(["ui".into()])
    );
}

#[test]
fn parser_treats_zero_behind_by_as_no_rebase() {
    let raw = include_str!("fixtures/pr_details.json").replacen(
        "\"repository\": {",
        "\"repository\": {\"rebase1\": {\"compare\": {\"behindBy\": 0}},",
        1,
    );

    let details = parse_stack_response(&raw, &stack()).unwrap();

    assert!(details.remote_rebase_branches.is_empty());
}

#[test]
fn parser_treats_null_behind_by_as_no_rebase() {
    let raw = include_str!("fixtures/pr_details.json").replacen(
        "\"repository\": {",
        "\"repository\": {\"rebase1\": {\"compare\": {\"behindBy\": null}},",
        1,
    );

    let details = parse_stack_response(&raw, &stack()).unwrap();

    assert!(details.remote_rebase_branches.is_empty());
}

#[test]
fn parser_treats_missing_comparison_with_graphql_error_as_no_rebase() {
    let raw =
        r#"{"data":{"repository":{"rebase1":null}},"errors":[{"message":"comparison denied"}]}"#;

    let details = parse_stack_response(raw, &stack()).unwrap();

    assert!(details.remote_rebase_branches.is_empty());
}

#[test]
fn parser_rejects_responses_without_data_and_preserves_bot_and_context() {
    let error = parse_stack_response(
        r#"{"data":null,"errors":[{"message":"bad query"}]}"#,
        &stack(),
    )
    .unwrap_err();
    assert!(matches!(error, GitHubError::Parse(_)));
    let details = parse_stack_response(include_str!("fixtures/pr_details.json"), &stack()).unwrap();
    let draft = details.prs.iter().find(|pr| pr.number == 43).unwrap();
    assert_eq!(draft.unresolved_threads, 2);
    assert!(draft.reviews[0].is_bot);
    assert_eq!(
        draft
            .checks
            .iter()
            .find(|c| c.name == "coverage")
            .unwrap()
            .conclusion
            .as_deref(),
        Some("FAILURE")
    );
}

#[test]
fn classifier_distinguishes_local_tracking_from_remote_discovery() {
    for action in ["push", "sync"] {
        assert_eq!(
            classify_gh_failure(&["stack", action], 2, "not in a stack"),
            GitHubError::LocalTrackingMissing
        );
    }
    assert!(matches!(
        classify_gh_failure(&["api", "graphql"], 2, "query failed"),
        GitHubError::Command { code: 2, .. }
    ));
    assert_eq!(
        classify_gh_failure(&["stack", "sync"], 3, "resolve conflicts"),
        GitHubError::RebaseConflict
    );
    assert_eq!(
        classify_gh_failure(&["stack", "rebase"], 7, "in progress"),
        GitHubError::RebaseConflict
    );
    assert!(matches!(
        classify_gh_failure(&["api"], 1, "API rate limit exceeded"),
        GitHubError::RateLimited { .. }
    ));
    assert_eq!(
        classify_gh_failure(&["pr", "list"], 1, "gh auth login first"),
        GitHubError::Auth
    );
    assert!(matches!(
        classify_gh_failure(&["api"], 1, "connection refused"),
        GitHubError::Network(_)
    ));
    assert!(matches!(
        classify_gh_failure(&["api"], 5, "unknown"),
        GitHubError::Command { code: 5, .. }
    ));
}

#[test]
fn classifier_reports_missing_extension_only_for_stack_actions() {
    let stderr = "unknown command \"stack\" for \"gh\"";
    for action in ["checkout", "push", "sync"] {
        let error = classify_gh_failure(&["stack", action], 1, stderr);
        assert_eq!(error, GitHubError::MissingStackExtension);
        assert!(
            error
                .to_string()
                .contains("gh extension install github/gh-stack")
        );
    }
    assert!(matches!(
        classify_gh_failure(&["api", "graphql"], 1, stderr),
        GitHubError::Command { .. }
    ));
    assert!(matches!(
        classify_gh_failure(&["stack", "push"], 1, "unknown command \"push\""),
        GitHubError::Command { .. }
    ));
}

#[test]
fn sync_aborted_with_exit_zero_is_not_reported_as_success() {
    let output = classify_sync_output("local: main ← auth\n", "remote: main ← other\nSync aborted");
    assert!(matches!(output, Err(GitHubError::Diverged(_))));
    let success = classify_sync_output("Synced stack", "warning: refreshed PR labels").unwrap();
    assert!(success.contains("Synced stack"));
    assert!(success.contains("refreshed PR labels"));
}

#[test]
fn graphql_exit_one_with_partial_data_keeps_accessible_pr() {
    let stack = stack();
    let details = classify_graphql_result(
        1,
        include_str!("fixtures/pr_partial.json"),
        "GraphQL: one PR inaccessible",
        &stack,
    )
    .unwrap();
    assert_eq!(
        details.prs.iter().map(|pr| pr.number).collect::<Vec<_>>(),
        [42]
    );
    let failure = classify_graphql_result(
        1,
        r#"{"data":null,"errors":[{"message":"denied"}]}"#,
        "GraphQL request failed",
        &stack,
    );
    assert!(failure.is_err());
}

#[tokio::test]
async fn no_pr_view_skips_github_detail_queries() {
    let temp = tempfile::tempdir().unwrap();
    let provider = CliGitHubProvider::new(
        temp.path().join("nonexistent-worktree-no-command-may-run"),
        "example".into(),
        "project".into(),
    );
    let view = StackViewResponse {
        mode: StackMode::Single,
        trunk: "main".into(),
        current_branch: "topic".into(),
        branches: vec![StackBranch {
            name: "topic".into(),
            is_merged: false,
            is_queued: false,
            pr: None,
        }],
    };

    let details = provider.pr_details(&view).await.unwrap();
    assert!(details.prs.is_empty());
    assert!(details.remote_rebase_branches.is_empty());
}

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

#[tokio::test]
async fn plain_push_uses_the_provider_worktree_and_existing_upstream() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let repo = temp.path().join("repo");
    let worktree = temp.path().join("topic-worktree");
    fs::create_dir(&repo).unwrap();
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "push.default", "simple"]);
    commit(&repo, "base");
    let main_sha = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-u", "origin", "main"]);
    git(
        &repo,
        &["worktree", "add", "-b", "topic", worktree.to_str().unwrap()],
    );
    git(&worktree, &["push", "-u", "origin", "topic"]);
    commit(&worktree, "topic change");
    let topic_sha = git(&worktree, &["rev-parse", "HEAD"]);
    let provider = CliGitHubProvider::new(worktree, "example".into(), "project".into());

    provider.push_branch().await.unwrap();

    assert_eq!(git(&remote, &["rev-parse", "refs/heads/topic"]), topic_sha);
    assert_eq!(git(&remote, &["rev-parse", "refs/heads/main"]), main_sha);
}

#[tokio::test]
async fn plain_push_preserves_git_upstream_failure_without_linking_a_stack() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&repo, &["init", "-b", "topic"]);
    git(&repo, &["config", "push.default", "simple"]);
    git(&repo, &["config", "push.autoSetupRemote", "false"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    commit(&repo, "base");
    let provider = CliGitHubProvider::new(repo.clone(), "example".into(), "project".into());

    let error = provider.push_branch().await.unwrap_err();

    assert!(matches!(error, GitHubError::Git(ref stderr) if stderr.contains("upstream")));
    assert!(git(&remote, &["for-each-ref", "refs/heads"]).is_empty());
    assert!(
        !git(&repo, &["config", "--local", "--list"])
            .lines()
            .any(|line| line.starts_with("branch."))
    );
}

#[tokio::test]
async fn rejected_plain_push_leads_with_the_rejection_and_keeps_git_output() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote.git");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(temp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    git(&repo, &["init", "-b", "topic"]);
    git(&repo, &["config", "push.default", "simple"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    commit(&repo, "base");
    commit(&repo, "published");
    git(&repo, &["push", "-u", "origin", "topic"]);
    git(&repo, &["reset", "--hard", "HEAD~1"]);
    commit(&repo, "rewritten");
    let provider = CliGitHubProvider::new(repo, "example".into(), "project".into());

    let GitHubError::Git(message) = provider.push_branch().await.unwrap_err() else {
        panic!("plain push failures are Git errors");
    };

    let headline = message.lines().next().unwrap();
    assert!(headline.starts_with("[rejected]"), "{message}");
    assert!(headline.contains("non-fast-forward"), "{message}");
    assert!(message.contains("failed to push some refs"), "{message}");
}
