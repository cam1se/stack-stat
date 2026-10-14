//! Stack and worktree identity at the top of the dashboard.

use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use stack_stat_core::{
    model::{StackMode, StackStatus, StackSummary},
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{app::App, theme::Theme};

mod summary;
use summary::{blocker_line, next_to_merge_lines, summary_line};

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    let state = app.state();
    let (identity, stack_summary) = match state.stack_status {
        StackStatus::Loading => (
            Line::styled(
                "loading…",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::BOLD),
            ),
            None,
        ),
        StackStatus::NotInStack => (
            Line::styled(
                "No attached branch",
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::BOLD),
            ),
            None,
        ),
        StackStatus::Ready => {
            let identity_label = match state.mode {
                StackMode::Single => {
                    if state
                        .branches
                        .iter()
                        .any(|branch| branch.pr_number.is_some() || branch.pr.is_some())
                    {
                        "single PR".to_owned()
                    } else {
                        "single branch".to_owned()
                    }
                }
                StackMode::Stacked => {
                    let layers = state.branches.len();
                    let layer_label = if layers == 1 { "layer" } else { "layers" };
                    format!("{layers} {layer_label}")
                }
            };
            let separator = || Span::styled(" · ", Style::default().fg(theme.muted));
            let mut spans = Vec::new();
            if !state.repo.is_empty() {
                spans.push(Span::styled(
                    state.repo.clone(),
                    Style::default()
                        .fg(theme.header_title)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(separator());
            }
            spans.push(Span::styled(
                state.trunk.clone(),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(separator());
            spans.push(Span::styled(
                identity_label,
                Style::default().fg(theme.muted),
            ));
            (Line::from(spans), Some(state.summary()))
        }
    };
    let chart_area = chart_area(area, stack_summary, !state.branches.is_empty());
    let text_width = text_width(area, chart_area);
    let branch = if state.worktree_branch.is_empty() {
        if state.stack_status == StackStatus::NotInStack {
            "detached or empty HEAD"
        } else {
            "loading…"
        }
    } else {
        &state.worktree_branch
    };
    let age = state
        .last_github_fetch
        .map_or_else(|| "loading…".into(), |at| relative_age(app.now(), at));
    let active_interval = if app.ci_active() {
        app.config().poll.github_active
    } else {
        app.config().poll.github
    };
    let stale = state
        .last_github_fetch
        .is_some_and(|at| super::animation::stale_after(app.now(), at, active_interval));
    let age_color = if stale {
        if app.animation().phase(20) < 0.5 {
            theme.warning
        } else {
            theme.muted
        }
    } else {
        theme.muted
    };
    let icon = if app.is_fetching() {
        app.animation().spinner_char()
    } else {
        '↻'
    };
    let status_line = Line::from(vec![
        Span::styled("worktree: ", Style::default().fg(theme.muted)),
        Span::styled(branch.to_owned(), Style::default().fg(theme.accent)),
        Span::styled(format!("    {icon} "), Style::default().fg(theme.muted)),
        Span::styled(age, Style::default().fg(age_color)),
    ]);
    let summary = match stack_summary {
        Some(summary) => summary_line(summary, theme),
        None => match state.stack_status {
            StackStatus::Loading => {
                Line::styled("Stack summary: loading…", Style::default().fg(theme.muted))
            }
            StackStatus::NotInStack => Line::styled(
                "Check out an attached branch to view its dashboard",
                Style::default().fg(theme.muted),
            ),
            StackStatus::Ready => unreachable!("ready stacks have a summary"),
        },
    };
    // The pie chart already shows the verdict counts, so the count strip is only for headers
    // without one.
    let mut lines = vec![
        truncate_line(identity, text_width),
        truncate_line(status_line, text_width),
    ];
    if chart_area.is_none() {
        lines.push(truncate_line(summary, text_width));
    }
    if area.height >= 8 && stack_summary.is_some() {
        // Fill the rows beside the pie: the next layer, one bullet per issue, then the category
        // breakdown. Spare rows become spacers, first before `Blocked by:`, then before `Next:`;
        // surplus bullets collapse into a `+N more` line.
        let budget = usize::from(area.height.saturating_sub(2));
        let (head, bullets) = next_to_merge_lines(state, theme);
        let blocked = blocker_line(state, theme);
        let room = budget.saturating_sub(lines.len() + 1 + usize::from(blocked.is_some()));
        let spare = room.saturating_sub(bullets.len());
        let spacer_before_blocked = blocked.is_some() && spare >= 1;
        let spacer_before_next = spare > usize::from(blocked.is_some());
        if spacer_before_next {
            lines.push(Line::default());
        }
        lines.push(truncate_line(head, text_width));
        let shown = if bullets.len() <= room {
            bullets
        } else if room > 0 {
            let keep = room - 1;
            let more = bullets.len() - keep;
            let mut shown: Vec<_> = bullets.into_iter().take(keep).collect();
            shown.push(Line::styled(
                format!("  … +{more} more"),
                Style::default().fg(theme.muted),
            ));
            shown
        } else {
            Vec::new()
        };
        lines.extend(
            shown
                .into_iter()
                .map(|line| truncate_line(line, text_width)),
        );
        if let Some(blocked) = blocked {
            if spacer_before_blocked {
                lines.push(Line::default());
            }
            lines.push(truncate_line(blocked, text_width));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(super::title(" stack-stat ", theme))
        .border_style(Style::default().fg(theme.border));
    frame.render_widget(Paragraph::new(lines).block(block), area);

    if let (Some(chart_area), Some(summary)) = (chart_area, stack_summary) {
        super::pie::render(frame, chart_area, summary, theme);
    }
}

fn chart_area(area: Rect, summary: Option<StackSummary>, has_branches: bool) -> Option<Rect> {
    if area.height < 8 || area.width < 70 || !has_branches || summary.is_none() {
        return None;
    }
    // Dots are square, so a pie twice as many columns as rows reads as a circle.
    let rows = area.height.saturating_sub(2);
    let columns = rows * 2;
    Some(Rect::new(
        area.right().saturating_sub(columns + 2),
        area.y.saturating_add(1),
        columns,
        rows,
    ))
}

fn text_width(area: Rect, chart_area: Option<Rect>) -> usize {
    let content_left = area.x.saturating_add(1);
    match chart_area {
        Some(chart_area) => usize::from(chart_area.x.saturating_sub(content_left)),
        None => usize::from(area.width.saturating_sub(2)),
    }
}

fn truncate_line(line: Line<'static>, width: usize) -> Line<'static> {
    let length = line
        .spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum::<usize>();
    if length <= width {
        return line;
    }
    if width == 0 {
        return Line::default();
    }

    let mut spans = Vec::new();
    let mut remaining = width - 1;
    let mut ellipsis_style = Style::default();
    for span in line.spans {
        let length = span.content.chars().count();
        ellipsis_style = span.style;
        if length <= remaining {
            remaining -= length;
            spans.push(span);
            continue;
        }
        if remaining > 0 {
            spans.push(Span::styled(
                span.content.chars().take(remaining).collect::<String>(),
                span.style,
            ));
        }
        break;
    }
    spans.push(Span::styled("…", ellipsis_style));
    Line::from(spans)
}

pub(super) fn relative_age(now: DateTime<Utc>, at: DateTime<Utc>) -> String {
    let seconds = (now - at).num_seconds().max(0);
    if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else {
        format!("{}h ago", seconds / 3600)
    }
}
