mod support;

use chrono::{TimeZone, Utc};
use stack_stat_core::{compose::StackOutcome, model::StackStatus, providers::GitHubError};
use stack_stat_tui::{message::Source, poll::fetch_github};
use support::{PrBuilder, TestScenario};

#[tokio::test]
async fn github_lookup_uses_current_git_branch_even_before_the_git_poller_refreshes() {
    let (_, handles) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("api")
        .build()
        .into_app();
    handles.git.set_current_branch("auth");
    let now = Utc.timestamp_opt(1_760_000_000, 0).single().unwrap();
    let snapshot = fetch_github(&handles.git, &handles.github, now)
        .await
        .unwrap();
    let StackOutcome::InStack(stack) = snapshot.stack else {
        panic!("fixture has a GitHub stack");
    };
    assert_eq!(stack.current_branch, "auth");
}

#[tokio::test]
async fn unreadable_current_git_branch_is_not_misreported_as_a_missing_github_stack() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["api"])
        .pr("api", PrBuilder::new(43))
        .current_branch("api")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    handles.git.fail_next("worktree became unavailable");
    handles.github.set_stack_error(GitHubError::NotInStack);
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().stack_status, StackStatus::Ready);
    assert_eq!(app.state().branches.len(), 1);
    assert!(
        app.notice()
            .unwrap()
            .text
            .contains("worktree became unavailable")
    );
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().stack_status, StackStatus::NotInStack);
    assert!(app.notice().is_none());
}
