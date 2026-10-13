//! Contextual key hints, command prompts, warnings, and update notices.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
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
    let status_color = if app.status_line().is_error {
        theme.error
    } else {
        theme.warning
    };
    let hints = match app.mode() {
        Mode::Normal => normal_hints(area.width, &status),
        mode => vec![(String::new(), mode_hint(mode, app))],
    };
    let mut spans = Vec::new();
    for (index, (key, label)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        if !key.is_empty() {
            spans.push(Span::styled(
                format!(" {key} "),
                Style::default()
                    .fg(theme.muted)
                    .bg(theme.selected)
                    .add_modifier(Modifier::REVERSED),
            ));
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            label.clone(),
            Style::default().fg(theme.muted),
        ));
    }
    if !status.is_empty() {
        if !spans.is_empty() {
            spans.push(Span::raw("  │  "));
        }
        let used = spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum::<usize>();
        let available = area.width.saturating_sub(used as u16) as usize;
        spans.push(Span::styled(
            truncate(&status, available),
            Style::default().fg(status_color),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn normal_hints(width: u16, status: &str) -> Vec<(String, String)> {
    let compact = width < 100 || !status.is_empty();
    let actions: &[Action] = if compact {
        &[Action::Down, Action::OpenPr, Action::Help, Action::Quit]
    } else {
        &[
            Action::Down,
            Action::Up,
            Action::DetailPageDown,
            Action::OpenPr,
            Action::Checkout,
            Action::Refresh,
            Action::Help,
            Action::Quit,
        ]
    };
    let capacity = usize::from(width);
    let reserved = status.chars().count() + usize::from(!status.is_empty()) * 5;
    let mut used = reserved;
    let mut selected = Vec::new();
    for action in actions.iter().rev() {
        let binding = KEYMAP
            .iter()
            .find(|binding| binding.action == *action)
            .expect("all footer actions have bindings");
        let key = key_label(binding.key);
        let hint_width = key.chars().count() + binding.help.chars().count() + 4;
        let separator = usize::from(!selected.is_empty()) * 2;
        if used + separator + hint_width <= capacity || selected.is_empty() {
            used += separator + hint_width;
            selected.push((key, binding.help.to_owned()));
        }
    }
    selected.reverse();
    selected
}

fn mode_hint<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    mode: Mode,
    app: &App<G, H, S>,
) -> String {
    match mode {
        Mode::Filter => format!("Filter: {} (Enter keep, Esc clear)", app.filter()),
        Mode::Confirm(_) => "y confirm  n cancel".into(),
        Mode::Help => "any key closes help".into(),
        Mode::Normal => unreachable!("normal mode uses key hints"),
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.into();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let prefix: String = text.chars().take(width - 1).collect();
    format!("{prefix}…")
}

pub(super) fn key_label(key: crate::action::KeySpec) -> String {
    let label = match key.code {
        crossterm::event::KeyCode::Char(ch) => ch.to_string(),
        crossterm::event::KeyCode::Enter => "Enter".into(),
        crossterm::event::KeyCode::Tab => "Tab".into(),
        crossterm::event::KeyCode::Up => "↑".into(),
        crossterm::event::KeyCode::Down => "↓".into(),
        crossterm::event::KeyCode::PageUp => "PgUp".into(),
        crossterm::event::KeyCode::PageDown => "PgDn".into(),
        crossterm::event::KeyCode::Home => "Home".into(),
        crossterm::event::KeyCode::End => "End".into(),
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
