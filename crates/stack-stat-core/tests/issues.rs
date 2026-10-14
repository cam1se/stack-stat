use stack_stat_core::model::{
    BlockerSummary, BranchState, CheckConclusion, CheckStatus, CiCheck, Issue, LocalCheckResult,
    LocalCheckState, LocalSync, PrInfo, PrLifecycle, RebaseNeed, Review, ReviewDecision,
    ReviewState, StackState, Verdict,
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

fn ready_branch(name: &str) -> BranchState {
    BranchState {
        name: name.into(),
        head_sha: "a".repeat(40),
        rebase: RebaseNeed::None,
        trunk_rebase_gates: false,
        is_current: false,
        local_sync: LocalSync::InSync,
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
fn issues_emit_blockers_before_waiting_with_counts() {
    let mut branch = ready_branch("feature");
    branch.rebase = RebaseNeed::Parent;
    branch.trunk_rebase_gates = true;
    branch.pr.as_mut().unwrap().is_draft = true;
    let pr = branch.pr.as_mut().unwrap();
    pr.checks = vec![
        check(CheckStatus::Completed, Some(CheckConclusion::Failure)),
        check(CheckStatus::InProgress, None),
    ];
    pr.review_decision = ReviewDecision::ChangesRequested;
    pr.unresolved_threads = 2;
    pr.threads_gate = true;
    pr.reviews.push(Review {
        author: "reviewer".into(),
        state: ReviewState::ChangesRequested,
        is_bot: false,
    });
    branch.local_checks = vec![
        local(LocalCheckState::Fail, false),
        local(LocalCheckState::Pass, true),
        local(LocalCheckState::Missing, false),
    ];

    assert_eq!(
        branch.issues(),
        vec![
            Issue::Draft,
            Issue::RebaseParent,
            Issue::FailingChecks(1),
            Issue::ChangesRequested,
            Issue::UnresolvedThreads(2),
            Issue::LocalFailed(1),
            Issue::LocalStale(1),
            Issue::RunningChecks(1),
            Issue::MissingLocal(1),
        ]
    );
}

#[test]
fn issues_emit_each_waiting_condition() {
    let mut running = ready_branch("running");
    running.pr.as_mut().unwrap().checks[0] = check(CheckStatus::Queued, None);
    assert_eq!(running.issues(), vec![Issue::RunningChecks(1)]);

    let mut required = ready_branch("required");
    required.pr.as_mut().unwrap().review_decision = ReviewDecision::ReviewRequired;
    assert_eq!(required.issues(), vec![Issue::ReviewRequired]);

    let mut pending = ready_branch("pending");
    let pr = pending.pr.as_mut().unwrap();
    pr.review_decision = ReviewDecision::None;
    pr.reviews.push(Review {
        author: "reviewer".into(),
        state: ReviewState::Pending,
        is_bot: false,
    });
    assert_eq!(pending.issues(), vec![Issue::ReviewPending]);

    let mut loading = ready_branch("loading");
    loading.pr = None;
    assert_eq!(loading.issues(), vec![Issue::LoadingDetails]);
}

#[test]
fn gated_issues_require_their_gates_and_neutral_layers_have_none() {
    let mut branch = ready_branch("feature");
    branch.rebase = RebaseNeed::Trunk;
    branch.pr.as_mut().unwrap().unresolved_threads = 3;
    assert_eq!(branch.issues(), Vec::<Issue>::new());

    branch.trunk_rebase_gates = true;
    branch.pr.as_mut().unwrap().threads_gate = true;
    assert_eq!(
        branch.issues(),
        vec![Issue::RebaseTrunk, Issue::UnresolvedThreads(3)]
    );

    for lifecycle in [PrLifecycle::Merged, PrLifecycle::Queued] {
        branch.lifecycle = lifecycle;
        assert_eq!(branch.issues(), Vec::<Issue>::new());
    }
}

#[test]
fn verdict_follows_issues() {
    let mut blocker = ready_branch("blocker");
    blocker.pr.as_mut().unwrap().is_draft = true;
    assert_eq!(blocker.verdict(), Verdict::NoGo);

    let mut waiting = ready_branch("waiting");
    waiting.pr = None;
    assert_eq!(waiting.verdict(), Verdict::Wait);

    assert_eq!(ready_branch("ready").verdict(), Verdict::Go);
}

#[test]
fn absent_pr_is_not_loading_or_ready_even_with_passing_local_checks() {
    let mut branch = ready_branch("no-pr");
    branch.pr_number = None;
    branch.pr = None;

    assert_eq!(branch.issues(), vec![Issue::NoPr]);
    assert_eq!(branch.verdict(), Verdict::Wait);
    branch.local_checks.clear();
    assert_eq!(branch.verdict(), Verdict::Wait);
}

#[test]
fn absent_pr_preserves_failed_stale_and_missing_local_issues() {
    let mut branch = ready_branch("no-pr");
    branch.pr_number = None;
    branch.pr = None;
    branch.local_checks = vec![
        local(LocalCheckState::Fail, false),
        local(LocalCheckState::Pass, true),
        local(LocalCheckState::Missing, false),
    ];

    assert_eq!(
        branch.issues(),
        vec![
            Issue::LocalFailed(1),
            Issue::LocalStale(1),
            Issue::NoPr,
            Issue::MissingLocal(1),
        ]
    );
    assert_eq!(branch.verdict(), Verdict::NoGo);
    let stack = StackState {
        branches: vec![branch],
        ..StackState::default()
    };
    assert_eq!(stack.summary().no_go, 1);
    assert_eq!(stack.blocker_summary().local, 1);
}

#[test]
fn known_pr_without_details_waits_before_evaluating_readiness() {
    let mut branch = ready_branch("loading");
    branch.pr = None;
    branch.rebase = RebaseNeed::Parent;
    branch.local_checks = vec![
        local(LocalCheckState::Fail, true),
        local(LocalCheckState::Missing, false),
    ];

    assert_eq!(branch.issues(), vec![Issue::LoadingDetails]);
    assert_eq!(branch.verdict(), Verdict::Wait);
}

#[test]
fn absent_pr_with_only_missing_local_checks_waits_without_losing_evidence() {
    let mut branch = ready_branch("no-pr");
    branch.pr_number = None;
    branch.pr = None;
    branch.local_checks = vec![local(LocalCheckState::Missing, false)];

    assert_eq!(branch.issues(), vec![Issue::NoPr, Issue::MissingLocal(1)]);
    assert_eq!(branch.verdict(), Verdict::Wait);
}

#[test]
fn next_to_merge_returns_lowest_open_layer() {
    let mut merged = ready_branch("merged");
    merged.lifecycle = PrLifecycle::Merged;
    let mut queued = ready_branch("queued");
    queued.lifecycle = PrLifecycle::Queued;
    let open = ready_branch("open");
    let higher_open = ready_branch("higher-open");
    let stack = StackState {
        branches: vec![merged.clone(), queued.clone(), open, higher_open],
        ..StackState::default()
    };
    assert_eq!(
        stack.next_to_merge().map(|branch| branch.name.as_str()),
        Some("open")
    );

    let stack = StackState {
        branches: vec![merged, queued],
        ..StackState::default()
    };
    assert_eq!(stack.next_to_merge(), None);
}

#[test]
fn next_to_merge_skips_no_pr_rows_but_keeps_known_prs_loading_details() {
    let mut no_pr = ready_branch("no-pr");
    no_pr.pr_number = None;
    no_pr.pr = None;
    let mut loading = ready_branch("loading");
    loading.pr = None;
    let mut stack = StackState {
        branches: vec![no_pr, loading, ready_branch("higher")],
        ..StackState::default()
    };

    assert_eq!(
        stack.next_to_merge().map(|branch| branch.name.as_str()),
        Some("loading")
    );
    stack.branches.truncate(1);
    assert_eq!(stack.next_to_merge(), None);
}

#[test]
fn blocker_summary_counts_open_layers_by_category() {
    let mut draft = ready_branch("draft");
    draft.pr.as_mut().unwrap().is_draft = true;

    let mut rebase_and_checks = ready_branch("rebase-checks");
    rebase_and_checks.rebase = RebaseNeed::Parent;
    rebase_and_checks.pr.as_mut().unwrap().checks[0] =
        check(CheckStatus::Completed, Some(CheckConclusion::Failure));

    let mut review_and_threads = ready_branch("review-threads");
    let pr = review_and_threads.pr.as_mut().unwrap();
    pr.review_decision = ReviewDecision::ReviewRequired;
    pr.unresolved_threads = 1;
    pr.threads_gate = true;

    let mut local_branch = ready_branch("local");
    local_branch.local_checks = vec![
        local(LocalCheckState::Fail, false),
        local(LocalCheckState::Pass, true),
        local(LocalCheckState::Missing, false),
    ];

    let mut ignored = ready_branch("ignored");
    ignored.lifecycle = PrLifecycle::Merged;
    ignored.pr.as_mut().unwrap().is_draft = true;
    ignored.rebase = RebaseNeed::Parent;

    let stack = StackState {
        branches: vec![
            draft,
            rebase_and_checks,
            review_and_threads,
            local_branch,
            ignored,
        ],
        ..StackState::default()
    };
    assert_eq!(
        stack.blocker_summary(),
        BlockerSummary {
            draft: 1,
            rebase: 1,
            checks: 1,
            reviews: 1,
            threads: 1,
            local: 1,
        }
    );
}
