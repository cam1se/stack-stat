//! Human-readable descriptions of branch readiness issues.

use stack_stat_core::model::Issue;

/// Describe one readiness issue for concise terminal presentation.
#[must_use]
pub(super) fn describe(issue: Issue) -> String {
    match issue {
        Issue::Draft => "draft".into(),
        Issue::RebaseParent => "rebase onto layer below".into(),
        Issue::RebaseTrunk => "behind trunk".into(),
        Issue::RebaseStacked => "stacked on a layer needing rebase".into(),
        Issue::FailingChecks(count) => count_label(count, "check", "failing"),
        Issue::ChangesRequested => "changes requested".into(),
        Issue::UnresolvedThreads(count) => count_label(count, "unresolved thread", ""),
        Issue::LocalFailed(count) => count_label(count, "local check", "failed"),
        Issue::LocalStale(count) => count_label(count, "stale local result", ""),
        Issue::LoadingDetails => "loading".into(),
        Issue::RunningChecks(count) => count_label(count, "check", "running"),
        Issue::ReviewRequired => "review required".into(),
        Issue::ReviewPending => "review pending".into(),
        Issue::MissingLocal(count) => count_label(count, "local result", "missing"),
    }
}

fn count_label(count: usize, noun: &str, suffix: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    if suffix.is_empty() {
        format!("{count} {noun}{plural}")
    } else {
        format!("{count} {noun}{plural} {suffix}")
    }
}
#[cfg(test)]
mod tests {
    use stack_stat_core::model::Issue;

    use super::describe;
    #[test]
    fn describes_issues_with_correct_plurality() {
        let descriptions = [
            (Issue::Draft, "draft"),
            (Issue::RebaseParent, "rebase onto layer below"),
            (Issue::RebaseTrunk, "behind trunk"),
            (Issue::RebaseStacked, "stacked on a layer needing rebase"),
            (Issue::FailingChecks(1), "1 check failing"),
            (Issue::FailingChecks(2), "2 checks failing"),
            (Issue::ChangesRequested, "changes requested"),
            (Issue::UnresolvedThreads(1), "1 unresolved thread"),
            (Issue::UnresolvedThreads(2), "2 unresolved threads"),
            (Issue::LocalFailed(1), "1 local check failed"),
            (Issue::LocalStale(2), "2 stale local results"),
            (Issue::LoadingDetails, "loading"),
            (Issue::RunningChecks(2), "2 checks running"),
            (Issue::ReviewRequired, "review required"),
            (Issue::ReviewPending, "review pending"),
            (Issue::MissingLocal(2), "2 local results missing"),
        ];
        for (issue, expected) in descriptions {
            assert_eq!(describe(issue), expected);
        }
    }
}
