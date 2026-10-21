//! Modal confirmation showing the exact commands a positive response will run.

use std::borrow::Cow;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use stack_stat_core::providers::ProviderAction;

use super::status_bar::truncate;
use crate::{app::Prompt, theme::Theme};

pub(super) fn render(frame: &mut Frame<'_>, area: Rect, prompt: &Prompt, theme: &Theme) {
    if area.width < 8 || area.height < 3 {
        return;
    }
    let width = area.width.saturating_sub(4).min(76);
    let inner_width = usize::from(width.saturating_sub(2));
    let mut lines: Vec<Line<'static>> = prompt
        .detail
        .lines()
        .flat_map(|detail| wrap(detail, inner_width))
        .map(|detail| Line::styled(detail, Style::default().fg(theme.muted)))
        .collect();
    lines.push(Line::from(""));
    for (index, action) in prompt.steps.iter().enumerate() {
        let prefix = format!("{}. ", index + 1);
        let command = command(action);
        let command = truncate(&command, inner_width.saturating_sub(prefix.len())).into_owned();
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(theme.muted)),
            Span::styled(command, Style::default().fg(theme.accent)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "Enter/y",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" confirm  ", Style::default().fg(theme.muted)),
        Span::styled(
            "Esc/n",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(theme.muted)),
    ]));
    let height = (lines.len() as u16).saturating_add(2).min(area.height);
    let modal = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, modal);
    let title = format!(" {}? ", prompt.title.trim_end_matches('?'));
    let block = Block::default()
        .borders(Borders::ALL)
        .title(super::title(
            truncate(&title, inner_width).into_owned(),
            theme,
        ))
        .border_style(Style::default().fg(theme.warning));
    frame.render_widget(Paragraph::new(lines).block(block), modal);
}

fn command(action: &ProviderAction) -> Cow<'_, str> {
    match action {
        ProviderAction::Checkout(target) => Cow::Owned(format!("gh stack checkout {target}")),
        ProviderAction::PushStack => Cow::Borrowed("gh stack push"),
        ProviderAction::PushBranch => Cow::Borrowed("git push"),
        ProviderAction::SyncStack => Cow::Borrowed("gh stack sync"),
        ProviderAction::RebaseStack => Cow::Borrowed("gh stack rebase"),
        ProviderAction::OpenPr(number) => Cow::Owned(format!("gh pr view {number} --web")),
        ProviderAction::ToggleDraft { number, make_ready } => Cow::Owned(format!(
            "gh pr ready {number}{}",
            if *make_ready { "" } else { " --undo" }
        )),
    }
}

/// Greedy word wrap by display width; an over-long word is truncated.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let word = truncate(word, width);
        let needed = Span::raw(current.as_str()).width()
            + usize::from(!current.is_empty())
            + Span::raw(word.as_ref()).width();
        if !current.is_empty() && needed > width {
            rows.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&word);
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}
