//! Evidence labels shared by compact and full branch rows.

use stack_stat_core::model::{
    BranchState, CheckHealth, LocalCheckState, LocalSync, PrLifecycle, RebaseNeed, ReviewDecision,
    ReviewState, ReviewSummary,
};

use crate::theme::Theme;

pub(super) fn worktree_marker(branch: &BranchState) -> (&'static str, &'static str) {
    if !branch.is_current {
        return (" ", "");
    }
    let suffix = match branch.local_sync {
        LocalSync::InSync => "",
        LocalSync::Ahead => "↑",
        LocalSync::Behind => "↓",
        LocalSync::Diverged => "⇅",
        LocalSync::Unpushed => "?",
    };
    ("*", suffix)
}

pub(super) fn lifecycle_label(
    branch: &BranchState,
    theme: &Theme,
) -> (String, ratatui::style::Color) {
    match branch.lifecycle {
        PrLifecycle::Merged => ("Merged".into(), theme.muted),
        PrLifecycle::Queued => ("Queued".into(), theme.success),
        PrLifecycle::Closed => ("Closed".into(), theme.muted),
        PrLifecycle::Open => match branch.pr.as_ref() {
            Some(pr) if pr.is_draft => ("Draft".into(), theme.error),
            Some(_) => ("Open".into(), theme.success),
            None if branch.pr_number.is_some() => ("◷".into(), theme.warning),
            None => (String::new(), theme.muted),
        },
    }
}

#[derive(Clone, Copy)]
pub(super) enum Health {
    Success,
    Failure,
    Running,
    Muted,
}
impl Health {
    pub(super) fn color(self, theme: &Theme) -> ratatui::style::Color {
        match self {
            Self::Success => theme.success,
            Self::Failure => theme.error,
            Self::Running => theme.warning,
            Self::Muted => theme.muted,
        }
    }
}

/// Row summary of CI checks. While anything is still running the spinner replaces the icon, keeping
/// the color of the worst state so a failing PR spins red and the icon returns when checks finish.
pub(super) fn check_label(branch: &BranchState, spinner: char) -> (String, Health) {
    let Some(pr) = &branch.pr else {
        return (missing_pr_label(branch), Health::Muted);
    };
    let summary = pr.check_summary();
    if summary.total == 0 {
        return ("—".into(), Health::Muted);
    }
    let (icon, health) = match summary.worst {
        CheckHealth::Failure => ('✖', Health::Failure),
        CheckHealth::Running => (spinner, Health::Running),
        CheckHealth::Success => ('✔', Health::Success),
        CheckHealth::Skipped | CheckHealth::None => ('░', Health::Muted),
    };
    // A merged or closed layer's CI is history: keep the glyph but drop the color and animation.
    if matches!(branch.lifecycle, PrLifecycle::Merged | PrLifecycle::Closed) {
        let icon = if summary.running > 0 { '◷' } else { icon };
        return (
            format!("{icon} {}/{}", summary.passed, summary.total),
            Health::Muted,
        );
    }
    let icon = if summary.running > 0 { spinner } else { icon };
    (
        format!("{icon} {}/{}", summary.passed, summary.total),
        health,
    )
}

pub(super) fn review_label(
    branch: &BranchState,
    show_bots: bool,
    expanded: bool,
    full: bool,
) -> (String, Health) {
    let Some(pr) = &branch.pr else {
        return (missing_pr_label(branch), Health::Muted);
    };
    let summary = pr.review_summary(show_bots);
    let summary = if pr.reviews.is_empty() {
        match pr.review_decision {
            ReviewDecision::Approved => ReviewSummary::Approved,
            ReviewDecision::ChangesRequested => ReviewSummary::ChangesRequested,
            ReviewDecision::ReviewRequired => ReviewSummary::Pending,
            ReviewDecision::None => summary,
        }
    } else {
        summary
    };
    let requested = pr
        .reviews
        .iter()
        .any(|review| review.state == ReviewState::Pending && (show_bots || !review.is_bot));
    let (icon, label, health) = if pr.review_decision == ReviewDecision::ReviewRequired
        && summary != ReviewSummary::ChangesRequested
    {
        (
            "◷",
            if requested { "pending" } else { "required" },
            Health::Running,
        )
    } else {
        match summary {
            ReviewSummary::Approved => ("✔", "approved", Health::Success),
            ReviewSummary::ChangesRequested => ("↺", "changes", Health::Failure),
            ReviewSummary::Pending => ("◷", "pending", Health::Running),
            ReviewSummary::None => ("—", "", Health::Muted),
        }
    };
    if !full {
        return (icon.into(), health);
    }
    if expanded {
        let names: Vec<_> = pr
            .reviews
            .iter()
            .filter(|review| show_bots || !review.is_bot)
            .map(|review| {
                let icon = match review.state {
                    ReviewState::Approved => "✔",
                    ReviewState::ChangesRequested => "↺",
                    ReviewState::Pending => "◷",
                    _ => "·",
                };
                format!("{icon}{}", review.author)
            })
            .collect();
        if !names.is_empty() {
            return (names.join(" "), health);
        }
    }
    (
        if label.is_empty() {
            icon.into()
        } else {
            format!("{icon} {label}")
        },
        health,
    )
}

pub(super) fn thread_label(branch: &BranchState) -> (String, Health) {
    let Some(pr) = &branch.pr else {
        return (missing_pr_label(branch), Health::Muted);
    };
    if pr.unresolved_threads == 0 {
        return ("✔ 0".into(), Health::Success);
    }
    if pr.threads_gate {
        return (format!("✖ {}", pr.unresolved_threads), Health::Failure);
    }
    (format!("◷ {}", pr.unresolved_threads), Health::Running)
}

pub(super) fn rebase_label(branch: &BranchState, trunk: &str, full: bool) -> (String, Health) {
    if branch.pr_number.is_none() && branch.pr.is_none() {
        return (String::new(), Health::Muted);
    }
    match branch.rebase {
        RebaseNeed::None => (String::new(), Health::Muted),
        RebaseNeed::Parent | RebaseNeed::Stacked => ("↻".into(), Health::Failure),
        RebaseNeed::Trunk => (
            if full {
                format!("↻ {trunk}")
            } else {
                "↻".into()
            },
            if branch.trunk_rebase_gates {
                Health::Failure
            } else {
                Health::Muted
            },
        ),
    }
}

pub(super) fn local_label(branch: &BranchState, full: bool) -> (String, Health) {
    if branch.local_checks.is_empty() {
        return ("—".into(), Health::Muted);
    }
    if !full {
        let health = if branch
            .local_checks
            .iter()
            .any(|check| check.state == LocalCheckState::Fail && !check.is_stale)
        {
            Health::Failure
        } else if branch
            .local_checks
            .iter()
            .any(|check| check.is_stale || check.state == LocalCheckState::Missing)
        {
            Health::Running
        } else if branch
            .local_checks
            .iter()
            .any(|check| check.state == LocalCheckState::Pass)
        {
            Health::Success
        } else {
            Health::Muted
        };
        let icon = match health {
            Health::Failure => "✖",
            Health::Running => "◷",
            Health::Success => "✔",
            Health::Muted => "░░",
        };
        return (icon.into(), health);
    }
    let items: Vec<_> = branch
        .local_checks
        .iter()
        .map(|check| {
            let icon = if check.is_stale {
                "◷"
            } else {
                match check.state {
                    LocalCheckState::Pass => "✔",
                    LocalCheckState::Fail => "✖",
                    LocalCheckState::Skip => "░░",
                    LocalCheckState::Missing => "◷",
                }
            };
            format!("{icon} {}", check.name)
        })
        .collect();
    let health = if branch
        .local_checks
        .iter()
        .any(|check| check.state == LocalCheckState::Fail && !check.is_stale)
    {
        Health::Failure
    } else if branch
        .local_checks
        .iter()
        .any(|check| check.is_stale || check.state == LocalCheckState::Missing)
    {
        Health::Running
    } else {
        Health::Success
    };
    (items.join(" "), health)
}

fn missing_pr_label(branch: &BranchState) -> String {
    if branch.pr_number.is_none() && branch.pr.is_none() {
        String::new()
    } else {
        "—".into()
    }
}
