use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};
use tachyonfx::{Effect, Interpolation, Motion, SimpleRng, fx};

use crate::{
    theme::{ColorDepth, Theme},
    ui::UiRegions,
};

use super::splash::draw_splash;

const DISSOLVE_MS: u32 = 360;
const STATIC_REVEAL_MS: u64 = 160;
const CHROME_REVEAL_MS: u32 = 460;
const ROW_REVEAL_MS: u32 = 360;
const MAX_ROW_DELAY_MS: u64 = 460;

pub(super) struct Reveal {
    pub(super) area: Rect,
    overlay: Buffer,
    dissolve: Effect,
    chrome_areas: Vec<Rect>,
    chrome_effects: Vec<Effect>,
    rows: Vec<RowReveal>,
    applied: Duration,
    basic: bool,
}

struct RowReveal {
    area: Rect,
    delay: Duration,
    effect: Option<Effect>,
}

impl Reveal {
    pub(super) fn new(
        area: Rect,
        layout: UiRegions,
        visible_branches: usize,
        theme: Theme,
    ) -> Self {
        let basic = theme.depth == ColorDepth::Basic16;
        let row_areas = branch_rows(layout.table, visible_branches);
        let chrome_areas = chrome_areas(layout, row_areas.len());
        let chrome_effects = if basic {
            Vec::new()
        } else {
            chrome_areas
                .iter()
                .copied()
                .map(|area| fade_and_sweep(area, theme))
                .collect()
        };
        let row_count = row_areas.len();
        let rows = row_areas
            .into_iter()
            .enumerate()
            .map(|(index, area)| RowReveal {
                area,
                delay: row_delay(index, row_count),
                effect: (!basic).then(|| sweep(area, ROW_REVEAL_MS, theme)),
            })
            .collect();
        let mut overlay = Buffer::empty(area);
        draw_splash(&mut overlay, area, theme);
        Self {
            area,
            overlay,
            dissolve: fx::dissolve_to(
                Style::default().fg(Color::Reset).bg(Color::Reset),
                (DISSOLVE_MS, Interpolation::QuadOut),
            )
            .with_rng(SimpleRng::new(0x1a7e_5eed)),
            chrome_areas,
            chrome_effects,
            rows,
            applied: Duration::ZERO,
            basic,
        }
    }

    pub(super) fn apply(&mut self, buffer: &mut Buffer, elapsed: Duration, theme: Theme) {
        let delta = elapsed.saturating_sub(self.applied);
        if self.basic && elapsed < Duration::from_millis(STATIC_REVEAL_MS) {
            for &area in &self.chrome_areas {
                hide_area(buffer, area, theme.active_base);
            }
        } else {
            for effect in &mut self.chrome_effects {
                if effect.running() {
                    effect.process(delta.into(), buffer, self.area);
                }
            }
        }
        for row in &mut self.rows {
            if elapsed <= row.delay {
                hide_area(buffer, row.area, theme.active_base);
                continue;
            }
            if let Some(effect) = &mut row.effect
                && effect.running()
            {
                let started = self.applied.max(row.delay);
                effect.process(elapsed.saturating_sub(started).into(), buffer, self.area);
            }
        }
        if self.dissolve.running() {
            self.dissolve
                .process(delta.into(), &mut self.overlay, self.area);
        }
        composite_overlay(buffer, &self.overlay, self.area);
        self.applied = elapsed;
    }
}

fn fade_and_sweep(area: Rect, theme: Theme) -> Effect {
    fx::parallel(&[
        fx::fade_from(
            theme.active_base,
            theme.active_base,
            (CHROME_REVEAL_MS, Interpolation::SineInOut),
        ),
        sweep(area, CHROME_REVEAL_MS, theme),
    ])
    .with_area(area)
}

fn sweep(area: Rect, duration_ms: u32, theme: Theme) -> Effect {
    fx::sweep_in(
        Motion::LeftToRight,
        12,
        0,
        theme.active_base,
        (duration_ms, Interpolation::QuadOut),
    )
    .with_area(area)
}

fn chrome_areas(layout: UiRegions, rows: usize) -> Vec<Rect> {
    let mut areas = Vec::with_capacity(6);
    areas.push(layout.header);
    areas.push(layout.status);
    if let Some(detail) = layout.detail {
        areas.push(detail);
    }
    let top_height = layout.table.height.min(4);
    if top_height > 0 {
        areas.push(Rect {
            height: top_height,
            ..layout.table
        });
    }
    let row_end = layout
        .table
        .y
        .saturating_add(4)
        .saturating_add((rows.min(u16::MAX as usize) as u16).saturating_mul(2));
    if row_end < layout.table.bottom() {
        areas.push(Rect {
            y: row_end,
            height: layout.table.bottom().saturating_sub(row_end),
            ..layout.table
        });
    }
    areas
}

fn branch_rows(table: Rect, visible_branches: usize) -> Vec<Rect> {
    let first_row = table.y.saturating_add(4);
    let capacity = usize::from(table.bottom().saturating_sub(first_row) / 2);
    (0..visible_branches.min(capacity))
        .map(|index| Rect {
            y: first_row.saturating_add((index as u16).saturating_mul(2)),
            height: 2,
            ..table
        })
        .collect()
}

fn row_delay(index: usize, rows: usize) -> Duration {
    if rows <= 1 {
        return Duration::ZERO;
    }
    Duration::from_millis(MAX_ROW_DELAY_MS * (rows - 1 - index) as u64 / (rows - 1) as u64)
}

fn hide_area(buffer: &mut Buffer, area: Rect, color: Color) {
    let area = area.intersection(buffer.area);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buffer[(x, y)];
            cell.set_char(' ');
            cell.set_fg(color);
            cell.set_bg(color);
        }
    }
}

fn composite_overlay(buffer: &mut Buffer, overlay: &Buffer, area: Rect) {
    let area = area.intersection(buffer.area).intersection(overlay.area);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &overlay[(x, y)];
            if cell.symbol() != " " || cell.fg != Color::Reset || cell.bg != Color::Reset {
                buffer[(x, y)] = cell.clone();
            }
        }
    }
}
