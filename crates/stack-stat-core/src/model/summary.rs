//! Derived check and review summaries.

use super::{
    BranchState, CheckConclusion, CheckHealth, CheckStatus, CheckSummary, LocalCheckState, PrInfo,
    PrLifecycle, ReviewDecision, ReviewState, ReviewSummary, StackState, StackSummary, Verdict,
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
            }
        }
        summary
    }
}

impl BranchState {
    /// Go/no-go readiness. Failing or stale evidence outranks outstanding evidence; unknown evidence
    /// is never `Go`. Reviews ignore the bot display filter so toggling it cannot change readiness.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        match self.lifecycle {
            PrLifecycle::Merged => return Verdict::Merged,
            PrLifecycle::Queued => return Verdict::Queued,
            PrLifecycle::Open => {}
        }
        let Some(pr) = &self.pr else {
            return Verdict::Wait;
        };
        let reviews = pr.review_summary(true);
        let checks = pr.check_summary().worst;
        if pr.is_draft
            || self.needs_rebase
            || checks == CheckHealth::Failure
            || pr.review_decision == ReviewDecision::ChangesRequested
            || reviews == ReviewSummary::ChangesRequested
            || self
                .local_checks
                .iter()
                .any(|local| local.state == LocalCheckState::Fail || local.is_stale)
        {
            return Verdict::NoGo;
        }
        let review_outstanding = pr.review_decision == ReviewDecision::ReviewRequired
            || (pr.review_decision == ReviewDecision::None && reviews == ReviewSummary::Pending);
        if checks == CheckHealth::Running
            || review_outstanding
            || self
                .local_checks
                .iter()
                .any(|local| local.state == LocalCheckState::Missing)
        {
            return Verdict::Wait;
        }
        Verdict::Go
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
