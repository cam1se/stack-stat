//! Derived check and review summaries.

use super::{
    CheckConclusion, CheckHealth, CheckStatus, CheckSummary, PrInfo, ReviewState, ReviewSummary,
};
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
            summary.total += 1;
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
