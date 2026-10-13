//! Centered loading wordmark drawn beneath startup effects.

use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect};
use tachyonfx::{CellFilter, Effect, Interpolation, Motion, SimpleRng, fx};

use crate::theme::{ColorDepth, Theme};

mod render;

use render::{
    SplashLayout, clear_wordmark, composite_wordmark, draw_splash as draw_splash_frame,
    draw_splash_chrome, draw_wordmark, splash_layout,
};

const COALESCE_MS: u32 = 1_400;

pub(super) fn draw_splash(buffer: &mut Buffer, area: Rect, theme: Theme) {
    draw_splash_frame(buffer, area, theme);
}

/// Cached splash layer and its startup-only effects.
pub(super) struct Splash {
    area: Rect,
    layout: SplashLayout,
    wordmark_layer: Buffer,
    coalesce: Effect,
    shimmer: Option<Effect>,
}

impl Splash {
    /// Create a splash layer sized for one terminal viewport.
    pub(super) fn new(area: Rect, theme: Theme) -> Self {
        let layout = splash_layout(area);
        let wordmark = layout.wordmark;
        let coalesce = fx::coalesce((COALESCE_MS, Interpolation::SineInOut))
            .with_rng(SimpleRng::new(0x5a17_cafe))
            .with_area(wordmark)
            .with_filter(CellFilter::NonEmpty);
        let shimmer = (theme.depth != ColorDepth::Basic16).then(|| {
            let glow = fx::hsl_shift(
                Some([0.0, 5.0, 14.0]),
                None,
                (620, Interpolation::SineInOut),
            );
            let sweep = fx::sweep_in(
                Motion::LeftToRight,
                14,
                0,
                theme.header_title,
                (620, Interpolation::SineInOut),
            );
            fx::repeating(fx::sequence(&[
                fx::parallel(&[glow, sweep]),
                fx::sleep(240),
            ]))
            .with_area(wordmark)
            .with_filter(CellFilter::NonEmpty)
        });
        Self {
            area,
            layout,
            wordmark_layer: Buffer::empty(area),
            coalesce,
            shimmer,
        }
    }

    /// Return whether this splash has the requested viewport geometry.
    pub(super) fn matches_area(&self, area: Rect) -> bool {
        self.area == area
    }

    /// Draw the animated splash over the dashboard buffer.
    pub(super) fn apply(
        &mut self,
        buffer: &mut Buffer,
        delta: Duration,
        elapsed: Duration,
        theme: Theme,
    ) {
        draw_splash_chrome(buffer, self.area, self.layout, elapsed, theme);
        clear_wordmark(&mut self.wordmark_layer, self.layout.wordmark);
        draw_wordmark(&mut self.wordmark_layer, self.layout, theme);
        if self.coalesce.running() {
            self.coalesce
                .process(delta.into(), &mut self.wordmark_layer, self.area);
        } else if let Some(shimmer) = &mut self.shimmer {
            shimmer.process(delta.into(), &mut self.wordmark_layer, self.area);
        }
        composite_wordmark(buffer, &self.wordmark_layer, self.layout.wordmark);
    }
}
