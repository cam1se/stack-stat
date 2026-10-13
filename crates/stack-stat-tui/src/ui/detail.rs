//! Selected branch detail pane with complete per-check and reviewer evidence.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use stack_stat_core::{
    model::{CheckConclusion, CheckStatus, LocalCheckState, PrLifecycle, ReviewState},
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use super::header::relative_age;
use crate::{app::App, theme::Theme};

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    let Some(branch) = app.selected_branch().and_then(|name| {
        app.state()
            .branches
            .iter()
            .find(|branch| branch.name == name)
    }) else {
        return;
    };
    let state = match branch.lifecycle {
        PrLifecycle::Merged => "Merged",
        PrLifecycle::Queued => "Queued",
        PrLifecycle::Open if branch.pr.as_ref().is_some_and(|pr| pr.is_draft) => "Draft",
        PrLifecycle::Open if branch.pr.is_some() => "Ready",
        PrLifecycle::Open => "No PR details",
    };
    let identity = branch.pr_number.map_or_else(
        || branch.name.clone(),
        |number| format!("PR #{number} · {}", branch.name),
    );
    let mut lines: Vec<Line<'static>> = vec![Line::from(format!(
        "{identity} · {state}{}",
        if branch.is_current {
            "   ← worktree checked out here"
        } else {
            ""
        }
    ))];
    if branch.needs_rebase {
        lines.push(Line::styled(
            "⚠ needs rebase",
            Style::default().fg(theme.warning),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Checks:"));
    if let Some(pr) = &branch.pr {
        if pr.checks.is_empty() {
            lines.push(Line::from("  — none reported"));
        }
        for check in &pr.checks {
            let mut spinner_buffer = [0; 4];
            let spinner: &str = app
                .animation()
                .spinner_char()
                .encode_utf8(&mut spinner_buffer);
            let (icon, suffix, color) = if check.skipped_by_config {
                ("░░", "skipped (config)".into(), theme.muted)
            } else {
                match check.status {
                    CheckStatus::Queued => (spinner, "queued".into(), theme.warning),
                    CheckStatus::InProgress => (
                        spinner,
                        check.started_at.map_or_else(
                            || "running".into(),
                            |at| {
                                format!(
                                    "running ({})",
                                    relative_age(app.now(), at).trim_end_matches(" ago")
                                )
                            },
                        ),
                        theme.warning,
                    ),
                    CheckStatus::Completed => match check.conclusion {
                        Some(CheckConclusion::Success) => (
                            "✅",
                            check.completed_at.map_or_else(
                                || "completed".into(),
                                |at| format!("completed {}", relative_age(app.now(), at)),
                            ),
                            theme.success,
                        ),
                        Some(CheckConclusion::Skipped | CheckConclusion::Neutral) => {
                            ("░░", "skipped".into(), theme.muted)
                        }
                        _ => ("❌", "failed".into(), theme.error),
                    },
                }
            };
            lines.push(Line::styled(
                format!("  {icon} {}  {suffix}", check.name),
                Style::default().fg(color),
            ));
        }
    } else {
        lines.push(Line::from("  — no PR details"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Reviews:"));
    if let Some(pr) = &branch.pr {
        let visible: Vec<_> = pr
            .reviews
            .iter()
            .filter(|review| app.show_bots() || !review.is_bot)
            .collect();
        if visible.is_empty() {
            lines.push(Line::from(match pr.review_decision {
                stack_stat_core::model::ReviewDecision::Approved => "  ✅ approved (GitHub)",
                stack_stat_core::model::ReviewDecision::ChangesRequested => {
                    "  🔄 changes requested (GitHub)"
                }
                stack_stat_core::model::ReviewDecision::ReviewRequired => "  ⏳ review required",
                stack_stat_core::model::ReviewDecision::None => "  — no visible reviews",
            }));
        }
        for review in visible {
            let (icon, status, color) = match review.state {
                ReviewState::Approved => ("✅", "approved", theme.success),
                ReviewState::ChangesRequested => ("🔄", "changes requested", theme.error),
                ReviewState::Pending => ("⏳", "pending", theme.warning),
                ReviewState::Commented => ("·", "commented", theme.muted),
                ReviewState::Dismissed => ("·", "dismissed", theme.muted),
            };
            lines.push(Line::styled(
                format!(
                    "  {icon} {}  {status}{}",
                    review.author,
                    if review.is_bot { " (bot)" } else { "" }
                ),
                Style::default().fg(color),
            ));
        }
    } else {
        lines.push(Line::from("  — no PR details"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Local checks:"));
    if branch.local_checks.is_empty() {
        lines.push(Line::from("  — none declared or reported"));
    }
    for check in &branch.local_checks {
        let (icon, suffix, color) = if check.is_stale {
            let short = check.sha.get(..7).unwrap_or(&check.sha);
            (
                "⏳",
                format!("stale (HEAD moved, was {short})"),
                theme.muted,
            )
        } else {
            match check.state {
                LocalCheckState::Pass => ("✅", "passed".into(), theme.success),
                LocalCheckState::Fail => ("❌", "failed".into(), theme.error),
                LocalCheckState::Skip => ("░░", "skipped".into(), theme.muted),
                LocalCheckState::Missing => ("⏳", "missing".into(), theme.warning),
            }
        };
        lines.push(Line::styled(
            format!("  {icon} {}  {suffix}", check.name),
            Style::default().fg(color),
        ));
    }
    let content_height = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let scroll = app
        .detail_scroll()
        .min(total.saturating_sub(content_height));
    let mut visible: Vec<_> = lines
        .into_iter()
        .skip(scroll)
        .take(content_height)
        .collect();
    if scroll + content_height < total && content_height > 0 {
        visible[content_height - 1] = Line::from("  … ↓ J/PgDn");
    }
    let title = if scroll > 0 {
        format!(" Details ↑ {scroll}/{total} ")
    } else {
        " Details · J/K/PgUp/PgDn scroll ".into()
    };
    frame.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(theme.muted)),
        ),
        area,
    );
}
