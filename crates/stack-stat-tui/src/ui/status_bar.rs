//! Separate confirmation prompts, contextual key hints, and typed notices.

use std::borrow::Cow;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use stack_stat_core::providers::{
    GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider,
};

use crate::{
    action::{Action, KEYMAP},
    app::{App, Mode, NoticeLevel, Prompt},
    theme::Theme,
};

pub(super) fn height(mode: &Mode) -> u16 {
    let Mode::Confirm(prompt) = mode else {
        return 1;
    };
    // Title, each detail line, ordered command steps, then independent hints/notice.
    let rows = 2 + prompt.detail.lines().count() + prompt.steps.iter().count();
    rows.min(usize::from(u16::MAX)) as u16
}

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    if area.is_empty() {
        return;
    }
    if let Mode::Confirm(prompt) = app.mode() {
        render_prompt(
            frame,
            Rect {
                height: area.height.saturating_sub(1),
                ..area
            },
            prompt,
            theme,
        );
    }
    let feedback_area = Rect {
        y: area.bottom().saturating_sub(1),
        height: 1,
        ..area
    };
    let (status, color, progress) = match app.notice() {
        Some(notice) => (
            Cow::Borrowed(notice.text.as_str()),
            match notice.level {
                NoticeLevel::Info => theme.muted,
                NoticeLevel::Warn => theme.warning,
                NoticeLevel::Error => theme.error,
                NoticeLevel::Progress => theme.accent,
            },
            notice.level == NoticeLevel::Progress,
        ),
        None => (
            app.update_available().map_or(Cow::Borrowed(""), |version| {
                Cow::Owned(format!(
                    "stack-stat {version} available — run stack-stat update"
                ))
            }),
            theme.muted,
            false,
        ),
    };
    let mut spans = Vec::new();
    match app.mode() {
        Mode::Normal => {
            let status_width = Span::raw(status.as_ref()).width() + usize::from(progress) * 2;
            for (index, (key, label)) in normal_hints(area.width, status_width)
                .into_iter()
                .enumerate()
            {
                if index > 0 {
                    spans.push(Span::raw("  "));
                }
                spans.push(Span::styled(
                    key,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(label, Style::default().fg(theme.muted)));
            }
        }
        mode => spans.push(Span::styled(
            mode_hint(mode, app),
            Style::default().fg(theme.muted),
        )),
    }
    if !status.is_empty() || progress {
        if !spans.is_empty() {
            spans.push(Span::raw("  │  "));
        }
        if progress {
            spans.push(Span::styled(
                app.animation().spinner_char().to_string(),
                Style::default().fg(color),
            ));
            spans.push(Span::raw(" "));
        }
        let used = spans.iter().map(Span::width).sum::<usize>();
        let available = usize::from(area.width).saturating_sub(used);
        spans.push(Span::styled(
            truncate(&status, available),
            Style::default().fg(color),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), feedback_area);
}

fn render_prompt(frame: &mut Frame<'_>, area: Rect, prompt: &Prompt, theme: &Theme) {
    if area.is_empty() {
        return;
    }
    let width = usize::from(area.width);
    let response = if prompt.title.ends_with('?') {
        " y/n"
    } else {
        "? y/n"
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(
            truncate(&prompt.title, width.saturating_sub(response.len())),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(response, Style::default().fg(theme.accent)),
    ])];
    lines.extend(
        prompt
            .detail
            .lines()
            .map(|detail| Line::styled(truncate(detail, width), Style::default().fg(theme.muted))),
    );
    for (index, action) in prompt.steps.iter().enumerate() {
        let command = command(action);
        let prefix = if index == 0 { "1. " } else { "2. " };
        let command = match truncate(&command, width.saturating_sub(prefix.len())) {
            Cow::Borrowed(_) => command,
            Cow::Owned(text) => Cow::Owned(text),
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(theme.muted)),
            Span::styled(command, Style::default().fg(theme.accent)),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
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

fn normal_hints(width: u16, status_width: usize) -> Vec<(String, &'static str)> {
    let compact = width < 100 || status_width > 0;
    let actions: &[Action] = if compact {
        &[Action::Down, Action::OpenPr, Action::Help, Action::Quit]
    } else {
        &[
            Action::Down,
            Action::Up,
            Action::DetailPageDown,
            Action::OpenPr,
            Action::Checkout,
            Action::Help,
            Action::Quit,
        ]
    };
    let capacity = usize::from(width);
    let reserved = status_width + usize::from(status_width > 0) * 5;
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
            selected.push((key, binding.help));
        }
    }
    selected.reverse();
    selected
}

fn mode_hint<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    mode: &Mode,
    app: &App<G, H, S>,
) -> Cow<'static, str> {
    match mode {
        Mode::Filter => Cow::Owned(format!("Filter: {} (Enter keep, Esc clear)", app.filter())),
        Mode::Confirm(_) => "y confirm  n cancel".into(),
        Mode::Help => "any key closes help".into(),
        Mode::Normal => unreachable!("normal mode uses key hints"),
    }
}

fn truncate(text: &str, width: usize) -> Cow<'_, str> {
    if Span::raw(text).width() <= width {
        return Cow::Borrowed(text);
    }
    if width == 0 {
        return Cow::Borrowed("");
    }
    let mut used = 0;
    let mut end = 0;
    for (index, ch) in text.char_indices() {
        let mut bytes = [0; 4];
        let char_width = Span::raw(&*ch.encode_utf8(&mut bytes)).width();
        if used + char_width > width - 1 {
            break;
        }
        used += char_width;
        end = index + ch.len_utf8();
    }
    let mut truncated = String::with_capacity(end + '…'.len_utf8());
    truncated.push_str(&text[..end]);
    truncated.push('…');
    Cow::Owned(truncated)
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
