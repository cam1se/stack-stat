//! Derived check and review summaries.

use super::{
    BlockerSummary, BranchState, CheckConclusion, CheckHealth, CheckStatus, CheckSummary, Issue,
    LocalCheckState, PrInfo, PrLifecycle, RebaseNeed, ReviewDecision, ReviewState, ReviewSummary,
    StackState, StackSummary, Verdict,
};

impl StackState {
    /// Count branches by verdict.
    #[must_use]
    pub fn summary(&self) -> StackSummary {
        let mut summary = StackSummary::default();
        for branch in &self.branches {
            match branch.verdict() {
                Verdict::Go => summary.go += 1,
                Verdict::Wait => summary.wait += 1,
                Verdict::NoGo => summary.no_go += 1,
                Verdict::Merged => summary.merged += 1,
                Verdict::Queued => summary.queued += 1,
                Verdict::Closed => summary.closed += 1,
            }
        }
        summary
    }

    /// Lowest open PR layer in stack order, even while its details are loading.
    #[must_use]
    pub fn next_to_merge(&self) -> Option<&BranchState> {
        self.branches
            .iter()
            .find(|branch| branch.lifecycle == PrLifecycle::Open && branch.pr_number.is_some())
    }

    /// Count open layers with blockers in each category.
    #[must_use]
    pub fn blocker_summary(&self) -> BlockerSummary {
        let mut summary = BlockerSummary::default();
        for branch in self
            .branches
            .iter()
            .filter(|branch| branch.lifecycle == PrLifecycle::Open)
        {
            let issues = branch.issues();
            summary.draft += issues.contains(&Issue::Draft) as usize;
            summary.rebase += issues.iter().any(|issue| {
                matches!(
                    issue,
                    Issue::RebaseParent | Issue::RebaseTrunk | Issue::RebaseStacked
                )
            }) as usize;
            summary.checks += issues
                .iter()
                .any(|issue| matches!(issue, Issue::FailingChecks(_)))
                as usize;
            summary.reviews += issues.iter().any(|issue| {
                matches!(
                    issue,
                    Issue::ChangesRequested | Issue::ReviewRequired | Issue::ReviewPending
                )
            }) as usize;
            summary.threads += issues
                .iter()
                .any(|issue| matches!(issue, Issue::UnresolvedThreads(_)))
                as usize;
            summary.local += issues.iter().any(|issue| {
                matches!(
                    issue,
                    Issue::LocalFailed(_) | Issue::LocalStale(_) | Issue::MissingLocal(_)
                )
            }) as usize;
        }
        summary
    }
}

impl BranchState {
    /// List the conditions affecting this branch's readiness.
    #[must_use]
    pub fn issues(&self) -> Vec<Issue> {
        if self.lifecycle != PrLifecycle::Open {
            return Vec::new();
        }
        if self.pr.is_none() && self.pr_number.is_some() {
            return vec![Issue::LoadingDetails];
        }

        let mut issues = Vec::new();
        let reviews = self
            .pr
            .as_ref()
            .map_or(ReviewSummary::None, |pr| pr.review_summary(true));
        if let Some(pr) = &self.pr {
            if pr.is_draft {
                issues.push(Issue::Draft);
            }
            match self.rebase {
                RebaseNeed::Parent => issues.push(Issue::RebaseParent),
                RebaseNeed::Stacked => issues.push(Issue::RebaseStacked),
                RebaseNeed::Trunk if self.trunk_rebase_gates => issues.push(Issue::RebaseTrunk),
                RebaseNeed::None | RebaseNeed::Trunk => {}
            }
            let failing_checks = pr
                .checks
                .iter()
                .filter(|check| !check.skipped_by_config)
                .filter(|check| {
                    check.status == CheckStatus::Completed
                        && !matches!(
                            check.conclusion,
                            Some(
                                CheckConclusion::Success
                                    | CheckConclusion::Neutral
                                    | CheckConclusion::Skipped
                            )
                        )
                })
                .count();
            if failing_checks > 0 {
                issues.push(Issue::FailingChecks(failing_checks));
            }
            if pr.review_decision == ReviewDecision::ChangesRequested
                || reviews == ReviewSummary::ChangesRequested
            {
                issues.push(Issue::ChangesRequested);
            }
            if pr.threads_gate && pr.unresolved_threads > 0 {
                issues.push(Issue::UnresolvedThreads(pr.unresolved_threads));
            }
        }
        let failed_local = self
            .local_checks
            .iter()
            .filter(|local| local.state == LocalCheckState::Fail)
            .count();
        if failed_local > 0 {
            issues.push(Issue::LocalFailed(failed_local));
        }
        let stale_local = self
            .local_checks
            .iter()
            .filter(|local| local.is_stale)
            .count();
        if stale_local > 0 {
            issues.push(Issue::LocalStale(stale_local));
        }

        if let Some(pr) = &self.pr {
            let running_checks = pr.check_summary().running;
            if running_checks > 0 {
                issues.push(Issue::RunningChecks(running_checks));
            }
            if pr.review_decision == ReviewDecision::ReviewRequired {
                issues.push(Issue::ReviewRequired);
            } else if pr.review_decision == ReviewDecision::None
                && reviews == ReviewSummary::Pending
            {
                issues.push(Issue::ReviewPending);
            }
        } else {
            issues.push(Issue::NoPr);
        }
        let missing_local = self
            .local_checks
            .iter()
            .filter(|local| local.state == LocalCheckState::Missing)
            .count();
        if missing_local > 0 {
            issues.push(Issue::MissingLocal(missing_local));
        }
        issues
    }

    /// Go/no-go readiness. Failing or stale evidence outranks outstanding evidence; unknown evidence
    /// is never `Go`. Reviews ignore the bot display filter so toggling it cannot change readiness.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        match self.lifecycle {
            PrLifecycle::Merged => return Verdict::Merged,
            PrLifecycle::Queued => return Verdict::Queued,
            PrLifecycle::Closed => return Verdict::Closed,
            PrLifecycle::Open => {}
        }
        let issues = self.issues();
        if issues.iter().any(|issue| issue.is_blocking()) {
            Verdict::NoGo
        } else if issues.is_empty() {
            Verdict::Go
        } else {
            Verdict::Wait
        }
    }
}

impl PrInfo {
    /// Summarize checks after applying repository skip rules.
    #[must_use]
    pub fn check_summary(&self) -> CheckSummary {
        let mut summary = CheckSummary {
            passed: 0,
            total: 0,
            worst: CheckHealth::None,
            running: 0,
        };
        for check in self.checks.iter().filter(|check| !check.skipped_by_config) {
            let health = match check.status {
                CheckStatus::Queued | CheckStatus::InProgress => CheckHealth::Running,
                CheckStatus::Completed => match check.conclusion {
                    Some(CheckConclusion::Success) => {
                        summary.passed += 1;
                        CheckHealth::Success
                    }
                    Some(CheckConclusion::Neutral | CheckConclusion::Skipped) => {
                        CheckHealth::Skipped
                    }
                    _ => CheckHealth::Failure,
                },
            };
            if health != CheckHealth::Skipped {
                summary.total += 1;
            }
            if health == CheckHealth::Running {
                summary.running += 1;
            }
            if priority(health) > priority(summary.worst) {
                summary.worst = health;
            }
        }
        summary
    }

    /// Summarize visible reviewers; changes requested outranks approval and pending.
    #[must_use]
    pub fn review_summary(&self, show_bots: bool) -> ReviewSummary {
        let mut summary = ReviewSummary::None;
        for review in self
            .reviews
            .iter()
            .filter(|review| show_bots || !review.is_bot)
        {
            let state = match review.state {
                ReviewState::ChangesRequested => ReviewSummary::ChangesRequested,
                ReviewState::Approved => ReviewSummary::Approved,
                ReviewState::Pending => ReviewSummary::Pending,
                ReviewState::Commented | ReviewState::Dismissed => ReviewSummary::None,
            };
            if review_priority(state) > review_priority(summary) {
                summary = state;
            }
        }
        summary
    }
}

const fn priority(health: CheckHealth) -> u8 {
    match health {
        CheckHealth::None => 0,
        CheckHealth::Skipped => 1,
        CheckHealth::Success => 2,
        CheckHealth::Running => 3,
        CheckHealth::Failure => 4,
    }
}

const fn review_priority(summary: ReviewSummary) -> u8 {
    match summary {
        ReviewSummary::None => 0,
        ReviewSummary::Pending => 1,
        ReviewSummary::Approved => 2,
        ReviewSummary::ChangesRequested => 3,
    }
}
