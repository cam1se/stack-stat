use stack_stat_core::providers::{GraphqlResponse, StackViewResponse};

#[test]
fn stack_view_fixture_preserves_order_and_flags() {
    let raw = include_str!("fixtures/stack_view.json");
    let stack: StackViewResponse = serde_json::from_str(raw).unwrap();
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
    assert!(stack.branches[2].needs_rebase);
    assert_eq!(stack.branches[1].pr.as_ref().unwrap().number, 43);
}

#[test]
fn unsubmitted_local_stack_can_omit_head_and_pr_fields() {
    let stack: StackViewResponse =
        serde_json::from_str(include_str!("fixtures/stack_view_no_pr.json")).unwrap();
    assert_eq!(stack.branches[0].name, "test-layer");
    assert!(stack.branches[0].pr.is_none());
}

#[test]
fn graphql_fixture_normalizes_checks_reviews_and_requests() {
    let raw = include_str!("fixtures/pr_details.json");
    let response: GraphqlResponse = serde_json::from_str(raw).unwrap();
    let prs = response.into_prs();
    assert_eq!(prs.len(), 2);
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
