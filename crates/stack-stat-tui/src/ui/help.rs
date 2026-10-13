//! Modal help generated from the same key bindings used by input dispatch.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::status_bar::key_label;
use crate::{action::KEYMAP, theme::Theme};

pub(super) fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    if area.width < 8 || area.height < 6 {
        return;
    }
    let width = area.width.saturating_sub(4).min(76);
    let height = ((KEYMAP.len() + 6) as u16).min(area.height.saturating_sub(2));
    let modal = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let mut lines = Vec::new();
    let mut group = "";
    for binding in KEYMAP {
        if binding.context != group {
            group = binding.context;
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::styled(group, Style::default().fg(theme.warning)));
        }
        lines.push(Line::from(format!(
            "  {:<8} {}",
            key_label(binding.key),
            binding.help
        )));
    }
    frame.render_widget(Clear, modal);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Keybindings · any key closes ")
                .border_style(Style::default().fg(theme.warning)),
        ),
        modal,
    );
}
