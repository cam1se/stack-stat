//! Modal help generated from the same key bindings used by input dispatch.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::status_bar::key_label;
use crate::{
    action::{KEYMAP, KeyBinding},
    theme::Theme,
};

pub(super) fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    if area.width < 8 || area.height < 6 {
        return;
    }
    let compact = area.height < 30 && area.width >= 70;
    let split = KEYMAP
        .iter()
        .position(|binding| binding.context == "Detail")
        .unwrap_or(KEYMAP.len());
    let left = build_lines(if compact { &KEYMAP[..split] } else { KEYMAP }, theme);
    let right = if compact {
        build_lines(&KEYMAP[split..], theme)
    } else {
        Vec::new()
    };
    let width = area.width.saturating_sub(4).min(76);
    let height = (left.len().max(right.len()) as u16 + 2).min(area.height);
    let modal = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keybindings · any key closes ")
        .border_style(Style::default().fg(theme.warning));
    frame.render_widget(block, modal);
    let inner = Rect {
        x: modal.x + 1,
        y: modal.y + 1,
        width: modal.width.saturating_sub(2),
        height: modal.height.saturating_sub(2),
    };
    if compact {
        let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);
        frame.render_widget(Paragraph::new(left), columns[0]);
        frame.render_widget(Paragraph::new(right), columns[1]);
    } else {
        frame.render_widget(Paragraph::new(left), inner);
    }
}

fn build_lines(bindings: &[KeyBinding], theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut group = "";
    for binding in bindings {
        if binding.context != group {
            group = binding.context;
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::styled(
                group.to_owned(),
                Style::default().fg(theme.warning),
            ));
        }
        lines.push(Line::from(format!(
            "  {:<8} {}",
            key_label(binding.key),
            binding.help
        )));
    }
    lines
}
