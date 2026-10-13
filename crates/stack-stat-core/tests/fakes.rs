use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use chrono::{TimeZone, Utc};
use stack_stat_core::providers::{
    CheckResult, FakeGitHubProvider, FakeGitProvider, GitHubError, GitHubProvider, GitProvider,
    MemoryStatusStore, ProviderAction, ReportedState, StackViewResponse, StatusEntry,
    StatusStoreProvider, retained,
};

fn stack() -> StackViewResponse {
    serde_json::from_str(include_str!("fixtures/stack_view.json")).unwrap()
}

fn entry(branch: &str, sha: &str, secs: i64) -> StatusEntry {
    StatusEntry {
        branch: branch.into(),
        sha: sha.into(),
        checks: BTreeMap::from([(
            "lint".into(),
            CheckResult {
                state: ReportedState::Pass,
                message: None,
                recorded_at: Utc.timestamp_opt(secs, 0).single().unwrap(),
            },
        )]),
    }
}

#[tokio::test]
async fn fake_git_reflects_branch_switch_without_reconstruction() {
    let git = FakeGitProvider::new(
        "auth",
        HashMap::from([("auth".into(), "aaa".into()), ("api".into(), "bbb".into())]),
        PathBuf::from("/tmp/common"),
    );
    let handle = git.clone();
    assert_eq!(git.current_branch().await.unwrap(), "auth");
    handle.set_current_branch("api");
    assert_eq!(git.current_branch().await.unwrap(), "api");
    assert_eq!(git.branch_shas().await.unwrap()["api"], "bbb");
}

#[tokio::test]
async fn fake_github_filters_prs_and_records_mutations() {
    let prs = serde_json::from_str::<stack_stat_core::providers::GraphqlResponse>(include_str!(
        "fixtures/pr_details.json"
    ))
    .unwrap()
    .into_prs();
    let github = FakeGitHubProvider::new(stack(), prs);
    assert_eq!(
        github
            .pr_details(&[43])
            .await
            .unwrap()
            .iter()
            .map(|pr| pr.number)
            .collect::<Vec<_>>(),
        [43]
    );
    github.toggle_draft(43, true).await.unwrap();
    github.checkout_branch("api").await.unwrap();
    github.push_stack().await.unwrap();
    github.sync_stack().await.unwrap();
    github.open_pr_in_browser(43).await.unwrap();
    assert_eq!(
        github.recorded_actions(),
        vec![
            ProviderAction::ToggleDraft {
                number: 43,
                make_ready: true
            },
            ProviderAction::Checkout("api".into()),
            ProviderAction::PushStack,
            ProviderAction::SyncStack,
            ProviderAction::OpenPr(43),
        ]
    );
    github.set_stack_error(GitHubError::NotInStack);
    assert_eq!(
        github.stack_view().await.unwrap_err(),
        GitHubError::NotInStack
    );
}

#[test]
fn memory_store_keeps_current_and_newest_stale_per_branch() {
    let store = MemoryStatusStore::default();
    let result = CheckResult {
        state: ReportedState::Pass,
        message: None,
        recorded_at: Utc.timestamp_opt(1, 0).single().unwrap(),
    };
    store.write("auth", "old", "lint", &result).unwrap();
    store
        .write(
            "auth",
            "newer",
            "lint",
            &CheckResult {
                recorded_at: Utc.timestamp_opt(2, 0).single().unwrap(),
                ..result.clone()
            },
        )
        .unwrap();
    store
        .write(
            "auth",
            "current",
            "lint",
            &CheckResult {
                recorded_at: Utc.timestamp_opt(3, 0).single().unwrap(),
                ..result.clone()
            },
        )
        .unwrap();
    store.write("gone", "old", "lint", &result).unwrap();
    assert_eq!(
        store
            .gc(&HashMap::from([("auth".into(), "current".into())]))
            .unwrap(),
        2
    );
    let remaining = store.read_all().unwrap();
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().any(|e| e.sha == "current"));
    assert!(remaining.iter().any(|e| e.sha == "newer"));
}

#[test]
fn pure_retention_helper_drops_deleted_branch_and_oldest_stale_entry() {
    let entries = vec![
        entry("auth", "a", 1),
        entry("auth", "b", 2),
        entry("auth", "c", 3),
        entry("gone", "z", 4),
    ];
    let (keep, delete) = retained(entries, &HashMap::from([("auth".into(), "c".into())]));
    assert_eq!(keep.len(), 2);
    assert_eq!(delete.len(), 2);
    assert!(keep.iter().any(|e| e.sha == "b"));
    assert!(keep.iter().any(|e| e.sha == "c"));
}
