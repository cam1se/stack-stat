use std::path::PathBuf;

use serde_json::{Value, json};
use stack_stat_core::{
    model::StackMode,
    providers::{
        CliGitHubProvider, DETACHED_HEAD, GitHubError, GitHubProvider, StackViewResponse,
        graphql::StackDiscovery,
    },
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/stack_view.json")).unwrap()
}

fn stack_node(response: &mut Value) -> &mut Value {
    &mut response["data"]["repository"]["openPullRequests"]["nodes"][0]["stack"]
}

fn discover(response: &Value, branch: &str) -> Result<StackViewResponse, GitHubError> {
    StackDiscovery::new(branch)?
        .accept_response(0, &response.to_string(), "")?
        .ok_or_else(|| GitHubError::Parse("test expected a complete stack".into()))
}

#[test]
fn unstacked_pr_is_a_single_layer_against_its_base() {
    let response = json!({"data": {"repository": {
        "defaultBranchRef": {"name": "main"},
        "openPullRequests": {"nodes": [{
            "number": 51,
            "url": "https://github.com/example/project/pull/51",
            "headRefName": "topic",
            "baseRefName": "release",
            "state": "OPEN",
            "isInMergeQueue": true,
            "headRepository": {"owner": {"login": "example"}},
            "stack": null
        }]},
        "pullRequests": {"nodes": []}
    }}});

    let view = discover(&response, "topic").unwrap();
    assert_eq!(view.mode, StackMode::Single);
    assert_eq!(view.trunk, "release");
    assert_eq!(view.current_branch, "topic");
    assert_eq!(view.branches.len(), 1);
    assert_eq!(view.branches[0].name, "topic");
    assert!(view.branches[0].is_queued);
    assert_eq!(view.branches[0].pr.as_ref().unwrap().number, 51);
}

#[test]
fn branch_without_pr_is_a_single_layer_against_repository_default() {
    let response = json!({"data": {"repository": {
        "defaultBranchRef": {"name": "develop"},
        "openPullRequests": {"nodes": []},
        "pullRequests": {"nodes": []}
    }}});

    let view = discover(&response, "topic").unwrap();
    assert_eq!(view.mode, StackMode::Single);
    assert_eq!(view.trunk, "develop");
    assert_eq!(view.branches.len(), 1);
    assert_eq!(view.branches[0].name, "topic");
    assert!(view.branches[0].pr.is_none());
    assert!(!view.branches[0].is_merged);
    assert!(!view.branches[0].is_queued);
}

#[test]
fn default_branch_without_pr_has_nothing_to_show() {
    let response = json!({"data": {"repository": {
        "defaultBranchRef": {"name": "develop"},
        "openPullRequests": {"nodes": []},
        "pullRequests": {"nodes": []}
    }}});

    assert_eq!(discover(&response, "develop"), Err(GitHubError::NotInStack));
}

fn entry(position: u32, branch: &str, state: &str, queued: bool) -> Value {
    json!({
        "position": position,
        "pullRequest": {
            "number": position + 40,
            "url": format!("https://github.com/example/project/pull/{}", position + 40),
            "headRefName": branch,
            "baseRefName": "main",
            "state": state,
            "isInMergeQueue": queued
        }
    })
}

#[test]
fn discovery_orders_by_position_despite_retargeted_merged_bases() {
    let mut response = fixture();
    let stack = stack_node(&mut response);
    stack["size"] = json!(4);
    stack["entries"]["totalCount"] = json!(4);
    stack["entries"]["nodes"] = json!([
        entry(4, "ui", "OPEN", false),
        entry(2, "auth", "MERGED", false),
        entry(3, "api", "OPEN", true),
        entry(1, "foundation", "MERGED", false)
    ]);

    let stack = discover(&response, "api").unwrap();

    assert_eq!(stack.trunk, "main");
    assert_eq!(stack.current_branch, "api");
    assert_eq!(
        stack
            .branches
            .iter()
            .map(|branch| branch.name.as_str())
            .collect::<Vec<_>>(),
        ["foundation", "auth", "api", "ui"]
    );
    assert_eq!(
        stack
            .branches
            .iter()
            .map(|branch| branch.is_merged)
            .collect::<Vec<_>>(),
        [true, true, false, false]
    );
    assert_eq!(
        stack
            .branches
            .iter()
            .map(|branch| branch.is_queued)
            .collect::<Vec<_>>(),
        [false, false, true, false]
    );
    let pr = stack.branches[2].pr.as_ref().unwrap();
    assert_eq!(pr.number, 43);
    assert_eq!(pr.url, "https://github.com/example/project/pull/43");
    assert_eq!(pr.state, "OPEN");
}

#[test]
fn one_entry_github_stack_remains_stacked_mode() {
    let mut response = fixture();
    let stack = stack_node(&mut response);
    stack["size"] = json!(1);
    stack["entries"]["totalCount"] = json!(1);
    stack["entries"]["nodes"] = json!([entry(1, "api", "OPEN", false)]);

    let view = discover(&response, "api").unwrap();
    assert_eq!(view.mode, StackMode::Stacked);
    assert_eq!(view.branches.len(), 1);
}

#[test]
fn malformed_single_pr_metadata_is_an_error_not_a_no_pr_row() {
    let valid = candidate(Some("example"), Value::Null);
    for field in [
        "number",
        "url",
        "headRefName",
        "baseRefName",
        "state",
        "isInMergeQueue",
        "headRepository",
        "stack",
    ] {
        let mut pr = valid.clone();
        pr.as_object_mut().unwrap().remove(field);
        let response = json!({"data": {"repository": {
            "defaultBranchRef": {"name": "main"},
            "openPullRequests": {"nodes": [pr]},
            "pullRequests": {"nodes": []}
        }}});
        assert!(
            matches!(discover(&response, "api"), Err(GitHubError::Parse(_))),
            "missing {field} must not become branch absence"
        );
    }
    for (field, value) in [
        ("number", json!(0)),
        ("number", json!("43")),
        ("url", json!("")),
        ("headRefName", json!("different-branch")),
        ("baseRefName", json!(" ")),
        ("state", json!("UNKNOWN")),
        ("isInMergeQueue", Value::Null),
        ("headRepository", json!({})),
    ] {
        let mut pr = valid.clone();
        pr[field] = value;
        let response = json!({"data": {"repository": {
            "defaultBranchRef": {"name": "main"},
            "openPullRequests": {"nodes": [pr]},
            "pullRequests": {"nodes": []}
        }}});
        assert!(
            matches!(discover(&response, "api"), Err(GitHubError::Parse(_))),
            "malformed {field} must remain an error"
        );
    }
}

#[test]
fn no_pr_row_requires_a_valid_repository_default_branch() {
    for default_branch in [
        Value::Null,
        json!({}),
        json!({"name": ""}),
        json!({"name": " "}),
        json!({"name": 1}),
    ] {
        let response = json!({"data": {"repository": {
            "defaultBranchRef": default_branch,
            "openPullRequests": {"nodes": []},
            "pullRequests": {"nodes": []}
        }}});
        assert!(matches!(
            discover(&response, "api"),
            Err(GitHubError::Parse(_))
        ));
    }
    let response = json!({"data": {"repository": {
        "openPullRequests": {"nodes": []},
        "pullRequests": {"nodes": []}
    }}});
    assert!(matches!(
        discover(&response, "api"),
        Err(GitHubError::Parse(_))
    ));
}

#[test]
fn single_pr_uses_its_base_even_when_repository_has_no_default_branch() {
    let response = json!({"data": {"repository": {
        "defaultBranchRef": null,
        "openPullRequests": {"nodes": [candidate(Some("example"), Value::Null)]},
        "pullRequests": {"nodes": []}
    }}});

    let view = discover(&response, "api").unwrap();
    assert_eq!(view.mode, StackMode::Single);
    assert_eq!(view.trunk, "main");
}

#[test]
fn discovery_prefers_newest_open_pr_over_newer_closed_or_merged_pr() {
    let mut response = fixture();
    let mut historical_stack = stack_node(&mut response).clone();
    historical_stack["baseRefName"] = json!("historical-trunk");
    response["data"]["repository"]["pullRequests"]["nodes"] =
        json!([candidate(Some("example"), historical_stack)]);

    assert_eq!(discover(&response, "api").unwrap().trunk, "main");
}

#[test]
fn discovery_does_not_resurrect_old_stack_when_newest_open_pr_has_no_stack() {
    let mut response = fixture();
    let historical_stack = stack_node(&mut response).clone();
    response["data"]["repository"]["pullRequests"]["nodes"] =
        json!([candidate(Some("example"), historical_stack)]);
    *stack_node(&mut response) = Value::Null;

    let view = discover(&response, "api").unwrap();
    assert_eq!(view.mode, StackMode::Single);
    assert_eq!(view.trunk, "main");
    assert_eq!(view.branches.len(), 1);
    assert_eq!(view.branches[0].pr.as_ref().unwrap().number, 43);
}

#[test]
fn discovery_falls_back_to_newest_lifecycle_pr_for_deleted_merged_branch() {
    let mut response = fixture();
    let mut merged_stack = stack_node(&mut response).clone();
    for node in merged_stack["entries"]["nodes"].as_array_mut().unwrap() {
        node["pullRequest"]["state"] = json!("MERGED");
    }
    response["data"]["repository"]["openPullRequests"]["nodes"] = json!([]);
    let mut merged_candidate = candidate(Some("example"), merged_stack);
    merged_candidate["headRefName"] = json!("auth");
    merged_candidate["state"] = json!("MERGED");
    response["data"]["repository"]["pullRequests"]["nodes"] = json!([merged_candidate]);

    let stack = discover(&response, "auth").unwrap();

    assert_eq!(stack.current_branch, "auth");
    assert!(stack.branches.iter().all(|branch| branch.is_merged));
    assert_eq!(stack.branches[0].name, "auth");
    assert_eq!(stack.branches[0].pr.as_ref().unwrap().state, "MERGED");
}

#[test]
fn newest_lifecycle_pr_with_null_stack_retains_merged_or_closed_state() {
    for state in ["MERGED", "CLOSED"] {
        let mut response = fixture();
        response["data"]["repository"]["openPullRequests"]["nodes"] = json!([]);
        response["data"]["repository"]["pullRequests"]["nodes"][0]["state"] = json!(state);

        let view = discover(&response, "api").unwrap();
        assert_eq!(view.mode, StackMode::Single);
        assert_eq!(view.branches[0].is_merged, state == "MERGED");
        assert_eq!(view.branches[0].pr.as_ref().unwrap().state, state);
    }
}

#[test]
fn lookup_query_selects_newest_open_and_lifecycle_with_escaped_branch_name() {
    let branch = "topic\"\\branch";
    let query = StackDiscovery::new(branch).unwrap().query("owner", "repo");
    let escaped = serde_json::to_string(branch).unwrap();

    assert!(query.contains("defaultBranchRef { name }"));
    assert_eq!(
        query
            .matches("number url headRefName baseRefName state isInMergeQueue")
            .count(),
        2
    );
    assert!(query.contains("openPullRequests: pullRequests("));
    assert!(query.contains("states:[OPEN]"));
    assert!(query.contains("states:[OPEN,MERGED,CLOSED]"));
    assert_eq!(query.matches("first:10,").count(), 2);
    assert_eq!(
        query.matches("headRepository { owner { login } }").count(),
        2
    );
    assert_eq!(
        query
            .matches("orderBy:{field:CREATED_AT,direction:DESC}")
            .count(),
        2
    );
    assert_eq!(query.matches(&format!("headRefName:{escaped}")).count(), 2);
    for field in [
        "baseRefName",
        "position",
        "isInMergeQueue",
        "headRefName",
        "size",
        "pageInfo",
    ] {
        assert!(query.contains(field), "missing {field}");
    }
}

fn candidate(owner: Option<&str>, stack: Value) -> Value {
    let head_repository = owner.map(|login| json!({"owner": {"login": login}}));
    json!({
        "number": 43,
        "url": "https://github.com/example/project/pull/43",
        "headRefName": "api",
        "baseRefName": "main",
        "state": "OPEN",
        "isInMergeQueue": false,
        "headRepository": head_repository,
        "stack": stack
    })
}

fn fork_stack(response: &mut Value) -> Value {
    let mut stack = stack_node(response).clone();
    stack["baseRefName"] = json!("fork-trunk");
    stack
}

fn discover_for(response: &Value, owner: &str) -> Result<StackViewResponse, GitHubError> {
    StackDiscovery::new("api")?
        .with_head_owner(owner)
        .accept_response(0, &response.to_string(), "")?
        .ok_or_else(|| GitHubError::Parse("test expected a complete stack".into()))
}

#[test]
fn fork_pr_with_the_same_branch_name_is_skipped_for_the_local_repository() {
    let mut response = fixture();
    let mine = stack_node(&mut response).clone();
    let theirs = fork_stack(&mut response);
    // The fork PR is newer, so it sorts first.
    response["data"]["repository"]["openPullRequests"]["nodes"] = json!([
        candidate(Some("alice"), theirs),
        candidate(Some("example"), mine)
    ]);

    assert_eq!(discover_for(&response, "example").unwrap().trunk, "main");
}

#[test]
fn fork_pr_alone_is_not_the_local_branch_stack() {
    let mut response = fixture();
    let theirs = fork_stack(&mut response);
    response["data"]["repository"]["openPullRequests"]["nodes"] =
        json!([candidate(Some("alice"), theirs.clone())]);
    response["data"]["repository"]["pullRequests"]["nodes"] =
        json!([candidate(Some("alice"), theirs)]);

    let view = discover_for(&response, "example").unwrap();
    assert_eq!(view.mode, StackMode::Single);
    assert_eq!(view.trunk, "main");
    assert_eq!(view.branches[0].name, "api");
    assert!(view.branches[0].pr.is_none());
}

#[test]
fn checkout_pushing_to_a_fork_selects_that_forks_pr() {
    let mut response = fixture();
    let upstream = stack_node(&mut response).clone();
    let mine = fork_stack(&mut response);
    response["data"]["repository"]["openPullRequests"]["nodes"] = json!([
        candidate(Some("someone-else"), upstream),
        candidate(Some("Alice"), mine)
    ]);

    // Owner comparison ignores case, as GitHub logins do.
    assert_eq!(
        discover_for(&response, "alice").unwrap().trunk,
        "fork-trunk"
    );
}

#[test]
fn lifecycle_fallback_skips_fork_prs_and_prs_from_deleted_forks() {
    let mut response = fixture();
    let mine = stack_node(&mut response).clone();
    let theirs = fork_stack(&mut response);
    response["data"]["repository"]["openPullRequests"]["nodes"] = json!([]);
    response["data"]["repository"]["pullRequests"]["nodes"] = json!([
        candidate(Some("alice"), theirs.clone()),
        candidate(None, theirs),
        candidate(Some("example"), mine)
    ]);

    assert_eq!(discover_for(&response, "example").unwrap().trunk, "main");
}

#[test]
fn open_pr_from_another_repository_does_not_block_own_merged_pr_fallback() {
    let mut response = fixture();
    let mine = stack_node(&mut response).clone();
    let theirs = fork_stack(&mut response);
    response["data"]["repository"]["openPullRequests"]["nodes"] =
        json!([candidate(Some("alice"), theirs)]);
    response["data"]["repository"]["pullRequests"]["nodes"] =
        json!([candidate(Some("example"), mine)]);

    assert_eq!(discover_for(&response, "example").unwrap().trunk, "main");
}

#[test]
fn discovery_fetches_every_entry_across_pages_before_returning_stack() {
    let mut response = fixture();
    let stack = stack_node(&mut response);
    stack["size"] = json!(101);
    stack["entries"]["totalCount"] = json!(101);
    stack["entries"]["nodes"] = json!(
        (1..=100)
            .rev()
            .map(|position| { entry(position, &format!("layer{position}"), "OPEN", false) })
            .collect::<Vec<_>>()
    );
    stack["entries"]["pageInfo"] = json!({"hasNextPage": true, "endCursor": "cursor100"});
    let mut continuation = stack.clone();
    continuation["entries"]["nodes"] = json!([entry(101, "layer101", "OPEN", false)]);
    continuation["entries"]["pageInfo"] = json!({"hasNextPage": false, "endCursor": "cursor101"});
    let continuation = json!({"data": {"node": continuation}});
    let mut discovery = StackDiscovery::new("layer101").unwrap();

    assert!(
        discovery
            .accept_response(0, &response.to_string(), "")
            .unwrap()
            .is_none()
    );
    let query = discovery.query("owner", "repo");
    assert!(query.contains("node(id:\"PRS_example\")"));
    assert!(query.contains("after:\"cursor100\""));
    let stack = discovery
        .accept_response(0, &continuation.to_string(), "")
        .unwrap()
        .unwrap();

    assert_eq!(stack.branches.len(), 101);
    assert_eq!(stack.branches[0].name, "layer1");
    assert_eq!(stack.branches[100].name, "layer101");
    assert_eq!(stack.current_branch, "layer101");
}

#[test]
fn discovery_rejects_truncated_or_inaccessible_entries_instead_of_partial_stack() {
    let mut truncated = fixture();
    stack_node(&mut truncated)["size"] = json!(4);
    assert!(matches!(
        discover(&truncated, "api"),
        Err(GitHubError::Parse(_))
    ));

    let mut inaccessible = fixture();
    stack_node(&mut inaccessible)["entries"]["nodes"][0]["pullRequest"] = Value::Null;
    assert!(matches!(
        discover(&inaccessible, "api"),
        Err(GitHubError::Parse(_))
    ));

    let mut null_entry = fixture();
    stack_node(&mut null_entry)["entries"]["nodes"][0] = Value::Null;
    assert!(matches!(
        discover(&null_entry, "api"),
        Err(GitHubError::Parse(_))
    ));
}

#[test]
fn discovery_rejects_missing_or_nonadvancing_pagination_cursor() {
    let mut response = fixture();
    let stack = stack_node(&mut response);
    stack["size"] = json!(10);
    stack["entries"]["totalCount"] = json!(10);
    stack["entries"]["pageInfo"] = json!({"hasNextPage": true, "endCursor": null});
    assert!(matches!(
        discover(&response, "api"),
        Err(GitHubError::Parse(_))
    ));

    stack_node(&mut response)["entries"]["pageInfo"]["endCursor"] = json!("cursor3");
    let continuation = json!({"data": {"node": stack_node(&mut response).clone()}});
    let mut discovery = StackDiscovery::new("api").unwrap();
    assert!(
        discovery
            .accept_response(0, &response.to_string(), "")
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        discovery.accept_response(0, &continuation.to_string(), ""),
        Err(GitHubError::Parse(_))
    ));
}

#[test]
fn failed_continuation_is_not_misclassified_as_branch_absence() {
    let mut response = fixture();
    let stack = stack_node(&mut response);
    stack["size"] = json!(4);
    stack["entries"]["totalCount"] = json!(4);
    stack["entries"]["pageInfo"] = json!({"hasNextPage": true, "endCursor": "cursor3"});
    let mut discovery = StackDiscovery::new("api").unwrap();
    assert!(
        discovery
            .accept_response(0, &response.to_string(), "")
            .unwrap()
            .is_none()
    );

    assert!(matches!(
        discovery.accept_response(0, r#"{"data":{"node":null}}"#, ""),
        Err(GitHubError::Parse(_))
    ));
}

#[test]
fn discovery_errors_are_not_stack_absence_even_with_null_stack_or_partial_data() {
    let mut response = fixture();
    response["errors"] = json!([{"message": "stack access denied"}]);
    assert!(matches!(
        discover(&response, "api"),
        Err(GitHubError::Parse(_))
    ));
    *stack_node(&mut response) = Value::Null;
    assert!(matches!(
        discover(&response, "api"),
        Err(GitHubError::Parse(_))
    ));
    response["data"]["repository"]["openPullRequests"]["nodes"] = json!([]);
    response["data"]["repository"]["pullRequests"]["nodes"] = json!([]);
    assert!(matches!(
        discover(&response, "api"),
        Err(GitHubError::Parse(_))
    ));

    for raw in [
        r#"{"data":{"repository":null}}"#,
        r#"{"data":null,"errors":[{"message":"query failed"}]}"#,
        r#"{"data":{"repository":{"openPullRequests":{"nodes":[{}]},"pullRequests":{"nodes":[]}}}}"#,
        "not json",
    ] {
        assert!(matches!(
            StackDiscovery::new("api")
                .unwrap()
                .accept_response(0, raw, ""),
            Err(GitHubError::Parse(_))
        ));
    }
}

#[test]
fn discovery_classifies_failed_requests_without_reporting_remote_absence() {
    for (stderr, expected) in [
        ("gh auth login first", GitHubError::Auth),
        (
            "connection refused",
            GitHubError::Network("connection refused".into()),
        ),
        (
            "API rate limit exceeded",
            GitHubError::RateLimited { reset_hint: None },
        ),
    ] {
        assert_eq!(
            StackDiscovery::new("api")
                .unwrap()
                .accept_response(1, "", stderr),
            Err(expected)
        );
    }
    assert!(matches!(
        StackDiscovery::new("api")
            .unwrap()
            .accept_response(2, "", "query failed"),
        Err(GitHubError::Command { code: 2, .. })
    ));
}

#[tokio::test]
async fn empty_and_detached_branch_need_no_github_command() {
    let github = CliGitHubProvider::new(
        PathBuf::from("nonexistent-worktree-no-command-may-run"),
        "example".into(),
        "project".into(),
    );

    for branch in ["", DETACHED_HEAD] {
        assert_eq!(
            github.stack_view(branch).await,
            Err(GitHubError::NotInStack)
        );
    }
}
