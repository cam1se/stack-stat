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
    model::{StackStatus, Verdict},
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{app::App, theme::Theme};

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    let state = app.state();
    let chain = match state.stack_status {
        StackStatus::Loading => "Stack: loading…".into(),
        StackStatus::NotInStack => "Stack: not in a stack".into(),
        StackStatus::Ready => {
            let mut chain = format!("Stack: ({})", state.trunk);
            for branch in &state.branches {
                chain.push_str(&format!(" ← {}", branch.name));
            }
            chain
        }
    };
    let branch = if state.worktree_branch.is_empty() {
        "loading…"
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
        ratatui::style::Color::Reset
    };
    let icon = if app.is_fetching() {
        app.animation().spinner_char()
    } else {
        '↻'
    };
    let status_line = Line::from(vec![
        Span::raw(format!("worktree: {branch}    {icon} ")),
        Span::styled(age, Style::default().fg(age_color)),
    ]);
    let summary = match state.stack_status {
        StackStatus::Ready => summary_line(app, theme),
        StackStatus::Loading => {
            Line::styled("Stack summary: loading…", Style::default().fg(theme.muted))
        }
        StackStatus::NotInStack => Line::styled(
            "Stack summary: not in a stack",
            Style::default().fg(theme.muted),
        ),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(super::title(" stack-stat ", theme))
        .border_style(Style::default().fg(theme.muted));
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(chain, Style::default().add_modifier(Modifier::BOLD)),
            status_line,
            summary,
        ])
        .block(block),
        area,
    );
}

fn summary_line<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    app: &App<G, H, S>,
    theme: &Theme,
) -> Line<'static> {
    let summary = app.state().summary();
    let mut spans = Vec::new();
    for (verdict, count, always_visible) in [
        (Verdict::Go, summary.go, true),
        (Verdict::Wait, summary.wait, true),
        (Verdict::NoGo, summary.no_go, true),
        (Verdict::Merged, summary.merged, false),
        (Verdict::Queued, summary.queued, false),
    ] {
        if !always_visible && count == 0 {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::raw("   "));
        }
        let (glyph, color) = super::verdict::style(verdict, theme);
        spans.push(Span::styled(
            format!("{glyph} {count}"),
            Style::default().fg(color),
        ));
    }
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
