use stack_stat_core::providers::{
    GitHubError, classify_graphql_result, classify_sync_output,
    graphql::{build_pr_query, classify_gh_failure, parse_pr_response},
};

#[test]
fn query_is_sorted_deduplicated_and_selects_check_and_review_fields() {
    let query = build_pr_query("example", "project", &[43, 42, 43]);
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
}

#[test]
fn parser_keeps_partial_data_and_omits_inaccessible_pr() {
    let prs = parse_pr_response(include_str!("fixtures/pr_partial.json")).unwrap();
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 42);
    assert!(prs[0].checks.is_empty());
    assert_eq!(prs[0].review_decision, None);
    assert_eq!(prs[0].unresolved_threads, 0);
    assert_eq!(prs[0].review_requests[0].author, "reviewers");
}

#[test]
fn parser_rejects_responses_without_data_and_preserves_bot_and_context() {
    let err = parse_pr_response(r#"{"data":null,"errors":[{"message":"bad query"}]}"#).unwrap_err();
    assert!(matches!(err, GitHubError::Parse(_)));
    let prs = parse_pr_response(include_str!("fixtures/pr_details.json")).unwrap();
    let draft = prs.iter().find(|pr| pr.number == 43).unwrap();
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
fn classifier_distinguishes_stack_state_conflict_and_network_failures() {
    assert_eq!(
        classify_gh_failure(&["stack", "view"], 2, "not in a stack"),
        GitHubError::NotInStack
    );
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
fn sync_aborted_with_exit_zero_is_not_reported_as_success() {
    let output = classify_sync_output("local: main ← auth\n", "remote: main ← other\nSync aborted");
    assert!(matches!(output, Err(GitHubError::Diverged(_))));
    let success = classify_sync_output("Synced stack", "warning: refreshed PR labels").unwrap();
    assert!(success.contains("Synced stack"));
    assert!(success.contains("refreshed PR labels"));
}

#[test]
fn graphql_exit_one_with_partial_data_keeps_accessible_pr() {
    let prs = classify_graphql_result(
        1,
        include_str!("fixtures/pr_partial.json"),
        "GraphQL: one PR inaccessible",
    )
    .unwrap();
    assert_eq!(prs.iter().map(|pr| pr.number).collect::<Vec<_>>(), [42]);
    let failure = classify_graphql_result(
        1,
        r#"{"data":null,"errors":[{"message":"denied"}]}"#,
        "GraphQL request failed",
    );
    assert!(failure.is_err());
}
