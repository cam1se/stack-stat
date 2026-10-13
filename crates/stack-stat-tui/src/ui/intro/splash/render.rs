//! Static splash composition and adaptive wordmark geometry.

use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use crate::theme::Theme;

const LARGE_MIN_WIDTH: u16 = 90;
const LARGE_MIN_HEIGHT: u16 = 28;
const WORDMARK_WIDTH: u16 = 59;
const WORDMARK_HEIGHT: u16 = 7;
const COMPACT_WORDMARK: &str = "stack-stat";
const TAGLINE: &str = "github pull request status";
const LOADING: &str = "loading";
const WORDMARK: [&str; WORDMARK_HEIGHT as usize] = [
    " ████ █████  ███   ████ █   █        ████ █████  ███  █████",
    "█       █   █   █ █     █  █        █       █   █   █   █  ",
    "█       █   █   █ █     █ █         █       █   █   █   █  ",
    " ████   █   █████ █     ██    █████  ████   █   █████   █  ",
    "    █   █   █   █ █     █ █             █   █   █   █   █  ",
    "    █   █   █   █ █     █  █            █   █   █   █   █  ",
    "████    █   █   █  ████ █   █       ████    █   █   █   █  ",
];

#[derive(Clone, Copy)]
pub(super) struct SplashLayout {
    pub(super) wordmark: Rect,
    tagline_y: u16,
    indicator_y: u16,
    large: bool,
}

pub(super) fn splash_layout(area: Rect) -> SplashLayout {
    let large = area.width >= LARGE_MIN_WIDTH && area.height >= LARGE_MIN_HEIGHT;
    let requested_width = if large {
        WORDMARK_WIDTH
    } else {
        COMPACT_WORDMARK.chars().count() as u16
    };
    let requested_height = if large { WORDMARK_HEIGHT } else { 1 };
    let wordmark = centered_rect(area, requested_width, requested_height);
    let group_height = requested_height.saturating_add(4).min(area.height);
    let group_y = area
        .y
        .saturating_add(area.height.saturating_sub(group_height) / 2);
    let max_logo_y = area.bottom().saturating_sub(wordmark.height);
    let wordmark = Rect {
        y: group_y.min(max_logo_y),
        ..wordmark
    };
    SplashLayout {
        wordmark,
        tagline_y: wordmark.bottom().saturating_add(1),
        indicator_y: wordmark.bottom().saturating_add(3),
        large,
    }
}

pub(super) fn draw_splash(buffer: &mut Buffer, area: Rect, theme: Theme) {
    let layout = splash_layout(area);
    draw_splash_chrome(buffer, area, layout, Duration::ZERO, theme);
    draw_wordmark(buffer, layout, theme);
}

pub(super) fn draw_splash_chrome(
    buffer: &mut Buffer,
    area: Rect,
    layout: SplashLayout,
    elapsed: Duration,
    theme: Theme,
) {
    let background = Style::default().bg(theme.active_base);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buffer[(x, y)];
            cell.reset();
            cell.set_style(background);
        }
    }
    draw_noise(buffer, layout.wordmark, elapsed, theme);
    draw_centered_text(
        buffer,
        area,
        layout.tagline_y,
        TAGLINE,
        Style::default().fg(theme.muted).bg(theme.active_base),
    );
    draw_loading(buffer, area, layout, elapsed, theme);
}

pub(super) fn draw_wordmark(buffer: &mut Buffer, layout: SplashLayout, theme: Theme) {
    let style = Style::default()
        .fg(theme.accent)
        .bg(theme.active_base)
        .add_modifier(Modifier::BOLD);
    if !layout.large {
        if layout.wordmark.y < buffer.area.bottom() {
            buffer.set_stringn(
                layout.wordmark.x,
                layout.wordmark.y,
                COMPACT_WORDMARK,
                layout.wordmark.width as usize,
                style,
            );
        }
        return;
    }
    for (row, line) in WORDMARK.iter().enumerate() {
        let y = layout.wordmark.y.saturating_add(row as u16);
        if y >= layout.wordmark.bottom() {
            break;
        }
        for (column, character) in line.chars().enumerate() {
            let x = layout.wordmark.x.saturating_add(column as u16);
            if x >= layout.wordmark.right() {
                break;
            }
            if character == '█' {
                set_cell(buffer, x, y, character, style);
            }
        }
    }
}

pub(super) fn clear_wordmark(buffer: &mut Buffer, wordmark: Rect) {
    let wordmark = wordmark.intersection(buffer.area);
    for y in wordmark.y..wordmark.bottom() {
        for x in wordmark.x..wordmark.right() {
            buffer[(x, y)].reset();
        }
    }
}

pub(super) fn composite_wordmark(buffer: &mut Buffer, layer: &Buffer, wordmark: Rect) {
    let wordmark = wordmark.intersection(buffer.area).intersection(layer.area);
    for y in wordmark.y..wordmark.bottom() {
        for x in wordmark.x..wordmark.right() {
            let cell = &layer[(x, y)];
            if cell.symbol() != " " {
                buffer[(x, y)] = cell.clone();
            }
        }
    }
}

fn centered_rect(area: Rect, requested_width: u16, requested_height: u16) -> Rect {
    let width = requested_width.min(area.width);
    let height = requested_height.min(area.height);
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    }
}

fn draw_noise(buffer: &mut Buffer, wordmark: Rect, elapsed: Duration, theme: Theme) {
    let phase = (elapsed.as_millis() / 90) as u32;
    let style = Style::default().fg(theme.border).bg(theme.active_base);
    for y in wordmark.y..wordmark.bottom() {
        for x in wordmark.x..wordmark.right() {
            let signal = u32::from(x)
                .wrapping_mul(73)
                .wrapping_add(u32::from(y).wrapping_mul(151))
                .wrapping_add(phase.wrapping_mul(97));
            if signal % 17 == 0 {
                set_cell(buffer, x, y, if signal & 1 == 0 { '·' } else { '░' }, style);
            }
        }
    }
}

fn draw_loading(
    buffer: &mut Buffer,
    area: Rect,
    layout: SplashLayout,
    elapsed: Duration,
    theme: Theme,
) {
    let y = layout.indicator_y;
    if y >= area.bottom() || area.width < 12 {
        return;
    }
    let prefix_width = LOADING.len() as u16 + 1;
    let desired_bar_width = if layout.large { 28 } else { 20 };
    let bar_width = desired_bar_width.min(area.width.saturating_sub(prefix_width));
    if bar_width == 0 {
        return;
    }
    let total_width = prefix_width.saturating_add(bar_width);
    let x = area
        .x
        .saturating_add(area.width.saturating_sub(total_width) / 2);
    buffer.set_stringn(
        x,
        y,
        LOADING,
        LOADING.len(),
        Style::default().fg(theme.muted).bg(theme.active_base),
    );
    let bar_x = x.saturating_add(prefix_width);
    let span = bar_width.saturating_sub(1);
    let step = (elapsed.as_millis() / 75) as u16;
    let position = if span == 0 {
        0
    } else {
        let cycle = span.saturating_mul(2);
        let phase = step % cycle;
        if phase <= span {
            phase
        } else {
            cycle.saturating_sub(phase)
        }
    };
    for offset in 0..bar_width {
        let active = offset.abs_diff(position) <= 1;
        set_cell(
            buffer,
            bar_x.saturating_add(offset),
            y,
            if active { '━' } else { '─' },
            Style::default()
                .fg(if active { theme.accent } else { theme.border })
                .bg(theme.active_base)
                .add_modifier(if active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        );
    }
}

fn draw_centered_text(buffer: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    if y >= area.bottom() {
        return;
    }
    let width = text.chars().count().min(area.width as usize) as u16;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    buffer.set_stringn(x, y, text, width as usize, style);
}

fn set_cell(buffer: &mut Buffer, x: u16, y: u16, character: char, style: Style) {
    if x < buffer.area.right() && y < buffer.area.bottom() {
        let cell = &mut buffer[(x, y)];
        cell.set_char(character);
        cell.set_style(style);
    }
}
