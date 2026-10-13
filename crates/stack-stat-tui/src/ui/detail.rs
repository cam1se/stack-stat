//! Selected branch detail pane with complete per-check and reviewer evidence.

use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use stack_stat_core::{
    model::{
        BranchState, CheckConclusion, CheckStatus, CiCheck, LocalCheckResult, LocalCheckState,
        PrLifecycle, RebaseNeed, Review, ReviewDecision, ReviewState,
    },
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{app::App, theme::Theme, wrap};

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

    let (glyph, verdict_color) = super::verdict::style(branch.verdict(), theme);
    let lifecycle = lifecycle_word(branch);
    let content_width = usize::from(area.width.saturating_sub(2));
    let mut header = Vec::new();
    if let Some(number) = branch.pr_number {
        header.push(Span::raw(format!("PR #{number} · ")));
    }
    header.extend([
        Span::styled(glyph, Style::default().fg(verdict_color)),
        Span::raw(" · "),
        Span::styled(
            lifecycle,
            Style::default().fg(if matches!(lifecycle, "Open" | "Queued") {
                theme.success
            } else {
                theme.muted
            }),
        ),
        Span::raw(if branch.is_current {
            "   * current worktree"
        } else {
            ""
        }),
    ]);
    let mut lines = vec![Line::from(header)];
    lines.extend(
        wrap::wrap(&branch.name, content_width)
            .into_iter()
            .map(|line| Line::styled(line, Style::default().add_modifier(Modifier::BOLD))),
    );
    if let Some(pr) = &branch.pr
        && !pr.title.is_empty()
    {
        lines.extend(
            wrap::wrap(&pr.title, content_width.saturating_sub(2))
                .into_iter()
                .map(|line| Line::styled(format!("  {line}"), Style::default().fg(theme.muted))),
        );
    }
    match branch.rebase {
        RebaseNeed::None => {}
        RebaseNeed::Parent => {
            let parent = app
                .state()
                .branches
                .iter()
                .position(|candidate| candidate.name == branch.name)
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| app.state().branches.get(index))
                .map_or("layer below", |parent| parent.name.as_str());
            lines.push(Line::styled(
                format!("Rebase needed: onto {parent}"),
                Style::default().fg(theme.error),
            ));
        }
        RebaseNeed::Trunk => {
            let blocking = branch.trunk_rebase_gates;
            lines.push(Line::styled(
                format!(
                    "Behind {}: rebase onto trunk{}",
                    app.state().trunk,
                    if blocking { "" } else { " (not blocking)" }
                ),
                Style::default().fg(if blocking { theme.error } else { theme.muted }),
            ));
        }
    }

    lines.push(Line::from(""));
    if let Some(pr) = &branch.pr {
        let counts = check_counts(&pr.checks);
        lines.push(Line::from(check_title(counts)));
        if pr.checks.is_empty() {
            lines.push(Line::from("  — none reported"));
        } else {
            let mut checks: Vec<_> = pr.checks.iter().collect();
            checks.sort_by_key(|check| check_group(check));
            for check in checks {
                lines.push(check_line(check, area.width, app, theme));
            }
        }
    } else {
        lines.push(Line::from("Checks:"));
        lines.push(Line::from("  — no PR details"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Reviews:"));
    if let Some(pr) = &branch.pr {
        let mut reviews: Vec<_> = pr
            .reviews
            .iter()
            .filter(|review| app.show_bots() || !review.is_bot)
            .collect();
        let (headline, headline_color) = review_headline(&reviews, pr.review_decision, theme);
        lines.push(Line::styled(
            format!("  {headline}"),
            Style::default().fg(headline_color),
        ));
        reviews.sort_by_key(|review| review_group(review));
        for review in reviews {
            lines.push(review_line(review, theme));
        }
    } else {
        lines.push(Line::from("  — no PR details"));
    }

    lines.push(Line::from(""));
    if let Some(pr) = &branch.pr {
        let color = if pr.unresolved_threads == 0 {
            theme.success
        } else if pr.threads_gate {
            theme.error
        } else {
            theme.warning
        };
        lines.push(Line::styled(
            format!("Threads: Unresolved threads: {}", pr.unresolved_threads),
            Style::default().fg(color),
        ));
    } else {
        lines.push(Line::styled(
            "Threads: — no PR details",
            Style::default().fg(theme.muted),
        ));
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Local checks:"));
    if branch.local_checks.is_empty() {
        lines.push(Line::from("  — none declared or reported"));
    } else {
        let mut checks: Vec<_> = branch.local_checks.iter().collect();
        checks.sort_by_key(|check| local_check_group(check));
        for check in checks {
            lines.push(local_check_line(check, theme));
        }
    }
    render_scrolled(frame, area, app, theme, lines);
}

fn lifecycle_word(branch: &BranchState) -> &'static str {
    match branch.lifecycle {
        PrLifecycle::Merged => "Merged",
        PrLifecycle::Queued => "Queued",
        PrLifecycle::Open if branch.pr.as_ref().is_some_and(|pr| pr.is_draft) => "Draft",
        PrLifecycle::Open if branch.pr.is_some() => "Open",
        PrLifecycle::Open => "No PR details",
    }
}

#[derive(Default)]
struct CheckCounts {
    failing: usize,
    running: usize,
    skipped: usize,
    passed: usize,
}

fn check_counts(checks: &[CiCheck]) -> CheckCounts {
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

fn check_title(counts: CheckCounts) -> String {
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
fn check_group(check: &CiCheck) -> u8 {
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

fn check_line<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
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

fn review_headline(
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

fn review_group(review: &Review) -> u8 {
    match review.state {
        ReviewState::ChangesRequested => 0,
        ReviewState::Pending => 1,
        ReviewState::Approved => 2,
        ReviewState::Commented | ReviewState::Dismissed => 3,
    }
}

fn review_line(review: &Review, theme: &Theme) -> Line<'static> {
    let (icon, status, color) = match review.state {
        ReviewState::Approved => ("✔", "approved", theme.success),
        ReviewState::ChangesRequested => ("↺", "changes requested", theme.error),
        ReviewState::Pending => ("…", "pending", theme.warning),
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

fn local_check_group(check: &LocalCheckResult) -> u8 {
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

fn local_check_line(check: &LocalCheckResult, theme: &Theme) -> Line<'static> {
    let (icon, suffix, color) = if check.is_stale {
        let short = check.sha.get(..7).unwrap_or(&check.sha);
        ("…", format!("stale (HEAD moved, was {short})"), theme.muted)
    } else {
        match check.state {
            LocalCheckState::Pass => ("✔", "passed".into(), theme.success),
            LocalCheckState::Fail => ("✖", "failed".into(), theme.error),
            LocalCheckState::Skip => ("░░", "skipped".into(), theme.muted),
            LocalCheckState::Missing => ("…", "missing".into(), theme.warning),
        }
    };
    Line::styled(
        format!("  {icon} {}  {suffix}", check.name),
        Style::default().fg(color),
    )
}

fn render_scrolled<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
    lines: Vec<Line<'static>>,
) {
    let content_height = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let max_scroll = total.saturating_sub(content_height);
    let scroll = app.detail_scroll().min(max_scroll);
    let visible: Vec<_> = lines
        .into_iter()
        .skip(scroll)
        .take(content_height)
        .collect();
    frame.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .title(super::title(" Details ", theme))
                .border_style(Style::default().fg(theme.muted)),
        ),
        area,
    );
    if max_scroll > 0 && content_height > 0 {
        let mut state = ScrollbarState::new(max_scroll)
            .position(scroll)
            .viewport_content_length(content_height);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::default().fg(theme.header_title)),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut state,
        );
    }
}
