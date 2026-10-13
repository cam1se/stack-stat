//! Stack and worktree identity at the top of the dashboard.

use chrono::{DateTime, Utc};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use stack_stat_core::{
    model::StackStatus,
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
    let line = format!("worktree: {branch}    ↻ {age}");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" stack-stat ")
        .border_style(Style::default().fg(theme.muted));
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(chain, Style::default().add_modifier(Modifier::BOLD)),
            Line::from(line),
        ])
        .block(block),
        area,
    );
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
