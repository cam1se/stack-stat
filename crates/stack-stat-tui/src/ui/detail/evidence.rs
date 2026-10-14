//! Formatting and ordering of PR and local-check evidence.

use ratatui::{
    style::{Color, Style},
    text::Line,
};
use stack_stat_core::{
    model::{
        BranchState, CheckConclusion, CheckStatus, CiCheck, LocalCheckResult, LocalCheckState,
        LocalSync, PrLifecycle, Review, ReviewDecision, ReviewState,
    },
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{app::App, theme::Theme};

pub(super) fn lifecycle_word(branch: &BranchState) -> &'static str {
    if branch.pr_number.is_none() && branch.pr.is_none() {
        return "no PR yet";
    }
    match branch.lifecycle {
        PrLifecycle::Merged => "Merged",
        PrLifecycle::Queued => "Queued",
        PrLifecycle::Open if branch.pr.as_ref().is_some_and(|pr| pr.is_draft) => "Draft",
        PrLifecycle::Open if branch.pr.is_some() => "Open",
        PrLifecycle::Open => "No PR details",
    }
}

pub(super) fn local_sync_text(local_sync: LocalSync) -> &'static str {
    match local_sync {
        LocalSync::InSync => "local in sync with remote",
        LocalSync::Ahead => "local ahead of remote",
        LocalSync::Behind => "local behind remote",
        LocalSync::Diverged => "local diverged from remote",
        LocalSync::Unpushed => "local branch not pushed",
    }
}

#[derive(Default)]
pub(super) struct CheckCounts {
    failing: usize,
    running: usize,
    skipped: usize,
    passed: usize,
}

pub(super) fn check_counts(checks: &[CiCheck]) -> CheckCounts {
    checks
        .iter()
        .fold(CheckCounts::default(), |mut counts, check| {
            match check_group(check) {
                0 => counts.failing += 1,
                1 => counts.running += 1,
                2 => counts.passed += 1,
                _ => counts.skipped += 1,
            }
            counts
        })
}

pub(super) fn check_title(counts: CheckCounts) -> String {
    let mut parts = vec!["Checks".to_owned()];
    for (count, label) in [
        (counts.failing, "failing"),
        (counts.running, "running"),
        (counts.passed, "passed"),
        (counts.skipped, "skipped"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    parts.join(" · ")
}

/// Display group: failing, running or queued, passed, then skipped.
pub(super) fn check_group(check: &CiCheck) -> u8 {
    if check.skipped_by_config {
        3
    } else {
        match check.status {
            CheckStatus::Queued | CheckStatus::InProgress => 1,
            CheckStatus::Completed => match check.conclusion {
                Some(CheckConclusion::Success) => 2,
                Some(CheckConclusion::Skipped | CheckConclusion::Neutral) => 3,
                _ => 0,
            },
        }
    }
}

pub(super) fn check_line<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    check: &CiCheck,
    width: u16,
    app: &App<G, H, S>,
    theme: &Theme,
) -> Line<'static> {
    let mut spinner_buffer = [0; 4];
    let spinner: &str = app
        .animation()
        .spinner_char()
        .encode_utf8(&mut spinner_buffer);
    let (icon, status, color) = if check.skipped_by_config {
        ("░░", "skipped (config)", theme.muted)
    } else {
        match check.status {
            CheckStatus::Queued => (spinner, "queued", theme.warning),
            CheckStatus::InProgress => (spinner, "running", theme.warning),
            CheckStatus::Completed => match check.conclusion {
                Some(CheckConclusion::Success) => ("✔", "passed", theme.success),
                Some(CheckConclusion::Skipped | CheckConclusion::Neutral) => {
                    ("░░", "skipped", theme.muted)
                }
                _ => ("✖", "failed", theme.error),
            },
        }
    };
    let duration = check_duration(check, app.now());
    let show_duration = width >= 46;
    let status_width = if show_duration { 18 } else { 16 };
    let used = 5 + status_width + usize::from(show_duration) * 9;
    let name_width = (width as usize).saturating_sub(used).clamp(4, 48);
    let name = truncate(&check.name, name_width);
    let mut text = format!("  {icon} {name:<name_width$}  {status:<status_width$}");
    if show_duration {
        text.push_str(&format!("  {duration:>7}"));
    }
    Line::styled(text, Style::default().fg(color))
}

fn check_duration(check: &CiCheck, now: chrono::DateTime<chrono::Utc>) -> String {
    let end = match check.status {
        CheckStatus::Completed => check.completed_at,
        CheckStatus::Queued => None,
        CheckStatus::InProgress => Some(now),
    };
    let Some(start) = check.started_at else {
        return "—".into();
    };
    let Some(end) = end else {
        return "—".into();
    };
    let seconds = (end - start).num_seconds();
    if seconds < 0 {
        return "—".into();
    }
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.into();
    }
    value
        .chars()
        .take(max.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}

pub(super) fn review_headline(
    reviews: &[&Review],
    decision: ReviewDecision,
    theme: &Theme,
) -> (String, Color) {
    let changes = reviews
        .iter()
        .filter(|review| review.state == ReviewState::ChangesRequested)
        .count();
    let approved = reviews
        .iter()
        .filter(|review| review.state == ReviewState::Approved)
        .count();
    let label = if changes > 0 || decision == ReviewDecision::ChangesRequested {
        "Changes requested"
    } else if reviews
        .iter()
        .any(|review| review.state == ReviewState::Pending)
    {
        "Review pending"
    } else if decision == ReviewDecision::ReviewRequired {
        "Review required"
    } else if approved > 0 || decision == ReviewDecision::Approved {
        "Approved"
    } else {
        "No visible reviews"
    };
    let color = if changes > 0 || decision == ReviewDecision::ChangesRequested {
        theme.error
    } else if reviews
        .iter()
        .any(|review| review.state == ReviewState::Pending)
        || decision == ReviewDecision::ReviewRequired
    {
        theme.warning
    } else if approved > 0 || decision == ReviewDecision::Approved {
        theme.success
    } else {
        theme.muted
    };
    (
        format!("{label} · {approved}/{} reviewers", reviews.len()),
        color,
    )
}

pub(super) fn review_group(review: &Review) -> u8 {
    match review.state {
        ReviewState::ChangesRequested => 0,
        ReviewState::Pending => 1,
        ReviewState::Approved => 2,
        ReviewState::Commented | ReviewState::Dismissed => 3,
    }
}

pub(super) fn review_line(review: &Review, theme: &Theme) -> Line<'static> {
    let (icon, status, color) = match review.state {
        ReviewState::Approved => ("✔", "approved", theme.success),
        ReviewState::ChangesRequested => ("↺", "changes requested", theme.error),
        ReviewState::Pending => ("◷", "pending", theme.warning),
        ReviewState::Commented => ("·", "commented", theme.muted),
        ReviewState::Dismissed => ("·", "dismissed", theme.muted),
    };
    Line::styled(
        format!(
            "  {icon} {}  {status}{}",
            review.author,
            if review.is_bot { " (bot)" } else { "" }
        ),
        Style::default().fg(color),
    )
}

pub(super) fn local_check_group(check: &LocalCheckResult) -> u8 {
    if check.is_stale
        || matches!(
            check.state,
            LocalCheckState::Fail | LocalCheckState::Missing
        )
    {
        0
    } else if check.state == LocalCheckState::Skip {
        2
    } else {
        1
    }
}

pub(super) fn local_check_line(check: &LocalCheckResult, theme: &Theme) -> Line<'static> {
    let (icon, suffix, color) = if check.is_stale {
        let short = check.sha.get(..7).unwrap_or(&check.sha);
        ("◷", format!("stale (HEAD moved, was {short})"), theme.muted)
    } else {
        match check.state {
            LocalCheckState::Pass => ("✔", "passed".into(), theme.success),
            LocalCheckState::Fail => ("✖", "failed".into(), theme.error),
            LocalCheckState::Skip => ("░░", "skipped".into(), theme.muted),
            LocalCheckState::Missing => ("◷", "missing".into(), theme.warning),
        }
    };
    Line::styled(
        format!("  {icon} {}  {suffix}", check.name),
        Style::default().fg(color),
    )
}
