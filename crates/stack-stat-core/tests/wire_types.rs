use stack_stat_core::{
    model::StackMode,
    providers::graphql::{StackDiscovery, parse_stack_response},
};

#[test]
fn github_stack_fixture_preserves_position_order_and_pr_identity() {
    let stack = StackDiscovery::new("api")
        .unwrap()
        .accept_response(0, include_str!("fixtures/stack_view.json"), "")
        .unwrap()
        .unwrap();
    assert_eq!(stack.trunk, "main");
    assert_eq!(stack.current_branch, "api");
    assert_eq!(
        stack
            .branches
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        ["auth", "api", "ui"]
    );
    assert!(stack.branches[0].is_merged);
    assert_eq!(stack.branches[1].pr.as_ref().unwrap().number, 43);
}

#[test]
fn branch_without_a_pull_request_is_a_single_mode_row() {
    let raw = r#"{"data":{"repository":{"defaultBranchRef":{"name":"main"},"openPullRequests":{"nodes":[]},"pullRequests":{"nodes":[]}}}}"#;
    let view = StackDiscovery::new("test-layer")
        .unwrap()
        .accept_response(0, raw, "")
        .unwrap()
        .unwrap();
    assert_eq!(view.mode, StackMode::Single);
    assert_eq!(view.trunk, "main");
    assert_eq!(view.branches[0].name, "test-layer");
    assert!(view.branches[0].pr.is_none());
}

#[test]
fn graphql_fixture_normalizes_checks_reviews_and_requests() {
    let raw = include_str!("fixtures/pr_details.json");
    let stack = StackDiscovery::new("api")
        .unwrap()
        .accept_response(0, include_str!("fixtures/stack_view.json"), "")
        .unwrap()
        .unwrap();
    let prs = parse_stack_response(raw, &stack).unwrap().prs;
    let draft = prs.iter().find(|pr| pr.number == 43).unwrap();
    assert!(draft.is_draft);
    assert!(
        draft
            .reviews
            .iter()
            .any(|review| review.author == "lint-bot" && review.is_bot)
    );
    assert!(
        draft
            .review_requests
            .iter()
            .any(|request| request.author == "bob")
    );
    let running = draft
        .checks
        .iter()
        .find(|check| check.name == "test")
        .unwrap();
    assert_eq!(running.status, "IN_PROGRESS");
    let status = draft
        .checks
        .iter()
        .find(|check| check.name == "coverage")
        .unwrap();
    assert_eq!(status.conclusion.as_deref(), Some("FAILURE"));
    assert_eq!(status.workflow, "");
}
