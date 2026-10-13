use stack_stat_core::model::{
    BranchState, CheckConclusion, CheckStatus, CiCheck, LocalCheckResult, LocalCheckState, PrInfo,
    PrLifecycle, Review, ReviewDecision, ReviewState, Verdict,
};

fn check(status: CheckStatus, conclusion: Option<CheckConclusion>) -> CiCheck {
    CiCheck {
        name: "ci".into(),
        workflow: String::new(),
        status,
        conclusion,
        started_at: None,
        completed_at: None,
        details_url: String::new(),
        skipped_by_config: false,
    }
}

fn local(state: LocalCheckState, is_stale: bool) -> LocalCheckResult {
    LocalCheckResult {
        name: "lint".into(),
        state,
        sha: String::new(),
        is_stale,
        message: None,
        recorded_at: None,
    }
}

fn ready_branch() -> BranchState {
    BranchState {
        name: "feat".into(),
        head_sha: "a".repeat(40),
        needs_rebase: false,
        is_current: false,
        lifecycle: PrLifecycle::Open,
        pr_number: Some(1),
        pr: Some(PrInfo {
            number: 1,
            url: String::new(),
            title: String::new(),
            is_draft: false,
            review_decision: ReviewDecision::Approved,
            unresolved_threads: 0,
            threads_gate: false,
            checks: vec![check(
                CheckStatus::Completed,
                Some(CheckConclusion::Success),
            )],
            reviews: Vec::new(),
        }),
        local_checks: vec![local(LocalCheckState::Pass, false)],
    }
}

#[test]
fn fully_green_branch_is_go() {
    assert_eq!(ready_branch().verdict(), Verdict::Go);
}

#[test]
fn gating_unresolved_threads_blocks_even_with_outstanding_evidence() {
    let mut branch = ready_branch();
    let pr = branch.pr.as_mut().unwrap();
    pr.unresolved_threads = 1;
    pr.threads_gate = true;
    pr.checks[0].status = CheckStatus::InProgress;
    pr.checks[0].conclusion = None;
    assert_eq!(branch.verdict(), Verdict::NoGo);
}

#[test]
fn non_gating_unresolved_threads_do_not_change_verdict() {
    let mut branch = ready_branch();
    let pr = branch.pr.as_mut().unwrap();
    pr.unresolved_threads = 1;
    assert_eq!(branch.verdict(), Verdict::Go);
}

#[test]
fn every_blocker_is_no_go_even_with_outstanding_evidence() {
    let blockers: [fn(&mut BranchState); 6] = [
        |b| b.pr.as_mut().unwrap().is_draft = true,
        |b| b.needs_rebase = true,
        |b| b.pr.as_mut().unwrap().checks[0].conclusion = Some(CheckConclusion::TimedOut),
        |b| b.pr.as_mut().unwrap().review_decision = ReviewDecision::ChangesRequested,
        |b| b.local_checks[0].state = LocalCheckState::Fail,
        |b| b.local_checks[0].is_stale = true,
    ];
    for (index, blocker) in blockers.iter().enumerate() {
        let mut branch = ready_branch();
        branch
            .pr
            .as_mut()
            .unwrap()
            .checks
            .push(check(CheckStatus::InProgress, None));
        blocker(&mut branch);
        assert_eq!(branch.verdict(), Verdict::NoGo, "blocker {index}");
    }
}

#[test]
fn outstanding_evidence_without_blockers_is_wait() {
    let mut running = ready_branch();
    running.pr.as_mut().unwrap().checks[0].status = CheckStatus::Queued;
    running.pr.as_mut().unwrap().checks[0].conclusion = None;
    assert_eq!(running.verdict(), Verdict::Wait);

    let mut review = ready_branch();
    review.pr.as_mut().unwrap().review_decision = ReviewDecision::ReviewRequired;
    assert_eq!(review.verdict(), Verdict::Wait);

    let mut missing = ready_branch();
    missing.local_checks[0].state = LocalCheckState::Missing;
    assert_eq!(missing.verdict(), Verdict::Wait);

    let mut no_pr = ready_branch();
    no_pr.pr = None;
    assert_eq!(no_pr.verdict(), Verdict::Wait);
}

#[test]
fn bot_changes_request_blocks_regardless_of_display_filter() {
    let mut branch = ready_branch();
    let pr = branch.pr.as_mut().unwrap();
    pr.review_decision = ReviewDecision::None;
    pr.reviews.push(Review {
        author: "bot".into(),
        state: ReviewState::ChangesRequested,
        is_bot: true,
    });
    assert_eq!(branch.verdict(), Verdict::NoGo);
}

#[test]
fn merged_and_queued_layers_are_neutral() {
    let mut merged = ready_branch();
    merged.lifecycle = PrLifecycle::Merged;
    merged.needs_rebase = true;
    assert_eq!(merged.verdict(), Verdict::Merged);
    let mut queued = ready_branch();
    queued.lifecycle = PrLifecycle::Queued;
    assert_eq!(queued.verdict(), Verdict::Queued);
}
