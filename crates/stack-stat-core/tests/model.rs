use stack_stat_core::model::{
    CheckConclusion, CheckHealth, CheckStatus, CiCheck, PrInfo, Review, ReviewDecision,
    ReviewState, ReviewSummary,
};

fn check(
    name: &str,
    status: CheckStatus,
    conclusion: Option<CheckConclusion>,
    skipped: bool,
) -> CiCheck {
    CiCheck {
        name: name.into(),
        workflow: "CI".into(),
        status,
        conclusion,
        started_at: None,
        completed_at: None,
        details_url: String::new(),
        skipped_by_config: skipped,
    }
}

fn pr(checks: Vec<CiCheck>, reviews: Vec<Review>) -> PrInfo {
    PrInfo {
        number: 42,
        url: "https://example.test/42".into(),
        title: String::new(),
        is_draft: false,
        review_decision: ReviewDecision::None,
        checks,
        reviews,
    }
}

#[test]
fn skipped_checks_are_not_in_count_or_worst_state() {
    let pr = pr(
        vec![
            check(
                "lint",
                CheckStatus::Completed,
                Some(CheckConclusion::Success),
                false,
            ),
            check("test", CheckStatus::InProgress, None, false),
            check(
                "noisy",
                CheckStatus::Completed,
                Some(CheckConclusion::Failure),
                true,
            ),
        ],
        vec![],
    );
    let summary = pr.check_summary();
    assert_eq!((summary.passed, summary.total), (1, 2));
    assert_eq!(summary.worst, CheckHealth::Running);
}

#[test]
fn failing_checks_override_running_and_passing() {
    let pr = pr(
        vec![
            check("test", CheckStatus::Queued, None, false),
            check(
                "build",
                CheckStatus::Completed,
                Some(CheckConclusion::Success),
                false,
            ),
            check(
                "security",
                CheckStatus::Completed,
                Some(CheckConclusion::Failure),
                false,
            ),
        ],
        vec![],
    );
    assert_eq!(pr.check_summary().worst, CheckHealth::Failure);
}

#[test]
fn changes_requested_overrides_approvals_and_pending_requests() {
    let pr = pr(
        vec![],
        vec![
            Review {
                author: "alice".into(),
                state: ReviewState::Approved,
                is_bot: false,
            },
            Review {
                author: "bob".into(),
                state: ReviewState::ChangesRequested,
                is_bot: false,
            },
            Review {
                author: "bot".into(),
                state: ReviewState::Pending,
                is_bot: true,
            },
        ],
    );
    assert_eq!(pr.review_summary(false), ReviewSummary::ChangesRequested);
    assert_eq!(pr.review_summary(true), ReviewSummary::ChangesRequested);
}

#[test]
fn bot_only_approval_is_hidden_when_bots_are_not_shown() {
    let pr = pr(
        vec![],
        vec![Review {
            author: "bot".into(),
            state: ReviewState::Approved,
            is_bot: true,
        }],
    );
    assert_eq!(pr.review_summary(false), ReviewSummary::None);
    assert_eq!(pr.review_summary(true), ReviewSummary::Approved);
}
