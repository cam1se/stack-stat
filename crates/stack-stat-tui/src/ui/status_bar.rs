//! Contextual key hints, command prompts, warnings, and update notices.

use std::fmt::Write as _;

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use stack_stat_core::providers::{GitHubProvider, GitProvider, StatusStoreProvider};

use crate::{
    action::{Action, KEYMAP},
    app::{App, Mode},
    theme::Theme,
};

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    let status = if app.status_line().text.is_empty() {
        app.update_available().map_or_else(String::new, |version| {
            format!("stack-stat {version} available — run stack-stat update")
        })
    } else {
        app.status_line().text.clone()
    };
    let hints = match app.mode() {
        Mode::Normal => {
            let compact = area.width < 100 || !status.is_empty();
            let actions: &[Action] = if compact {
                &[Action::Down, Action::OpenPr, Action::Help, Action::Quit]
            } else {
                &[
                    Action::Down,
                    Action::Up,
                    Action::OpenPr,
                    Action::Checkout,
                    Action::Refresh,
                    Action::Help,
                    Action::Quit,
                ]
            };
            let mut hints = String::new();
            for action in actions {
                if let Some(binding) = KEYMAP.iter().find(|binding| binding.action == *action) {
                    if !hints.is_empty() {
                        hints.push_str("  ");
                    }
                    let _ = write!(hints, "{} {}", key_label(binding.key), binding.help);
                }
            }
            hints
        }
        Mode::Filter => format!("Filter: {} (Enter keep, Esc clear)", app.filter()),
        Mode::Confirm(_) => "y confirm  n cancel".into(),
        Mode::Help => "any key closes help".into(),
    };
    let status_color = if app.status_line().is_error {
        theme.error
    } else {
        theme.warning
    };
    let line = Line::from(vec![
        Span::styled(hints, Style::default().fg(theme.muted)),
        Span::raw("  │  "),
        Span::styled(status, Style::default().fg(status_color)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

pub(super) fn key_label(key: crate::action::KeySpec) -> String {
    let label = match key.code {
        crossterm::event::KeyCode::Char(ch) => ch.to_string(),
        crossterm::event::KeyCode::Enter => "Enter".into(),
        crossterm::event::KeyCode::Tab => "Tab".into(),
        crossterm::event::KeyCode::Up => "↑".into(),
        crossterm::event::KeyCode::Down => "↓".into(),
        _ => format!("{:?}", key.code),
    };
    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        format!("Ctrl+{label}")
    } else {
        label
    }
}
