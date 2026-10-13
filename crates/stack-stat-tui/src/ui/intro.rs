//! Startup-only wordmark and dashboard reveal effects.

use std::time::Duration;

use ratatui::{buffer::Buffer, layout::Rect};

use crate::theme::Theme;

use super::UiRegions;

mod reveal;
mod splash;

use reveal::Reveal;
use splash::Splash;

const REVEAL_MS: u64 = 1_050;

/// Startup animation state driven by the runtime's frame deltas.
pub struct Intro {
    theme: Theme,
    phase: Phase,
    pending_delta: Duration,
    splash_elapsed: Duration,
    reveal_elapsed: Duration,
    splash: Option<Splash>,
}

enum Phase {
    Splash,
    Reveal(Option<Reveal>),
    Done,
}

impl Intro {
    /// Create the initial loading splash using the selected terminal theme.
    #[must_use]
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            phase: Phase::Splash,
            pending_delta: Duration::ZERO,
            splash_elapsed: Duration::ZERO,
            reveal_elapsed: Duration::ZERO,
            splash: None,
        }
    }

    /// Advance the startup state with a runtime-computed frame delta.
    pub fn advance(&mut self, delta: Duration, github_result_arrived: bool) {
        if matches!(self.phase, Phase::Done) {
            return;
        }
        if github_result_arrived && matches!(self.phase, Phase::Splash) {
            self.phase = Phase::Reveal(None);
            self.splash = None;
            self.reveal_elapsed = Duration::ZERO;
        }
        self.pending_delta = self.pending_delta.saturating_add(delta);
        match self.phase {
            Phase::Splash => {
                self.splash_elapsed = self.splash_elapsed.saturating_add(delta);
            }
            Phase::Reveal(_) => {
                self.reveal_elapsed = self.reveal_elapsed.saturating_add(delta);
                if self.reveal_elapsed >= Duration::from_millis(REVEAL_MS) {
                    self.phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
    }

    /// Apply the startup frame after the normal dashboard has rendered.
    pub fn apply(
        &mut self,
        buffer: &mut Buffer,
        area: Rect,
        layout: UiRegions,
        visible_branches: usize,
    ) {
        if area.width == 0 || area.height == 0 || matches!(self.phase, Phase::Done) {
            self.pending_delta = Duration::ZERO;
            return;
        }
        let delta = std::mem::take(&mut self.pending_delta);
        match &mut self.phase {
            Phase::Splash => {
                let needs_rebuild = self
                    .splash
                    .as_ref()
                    .is_none_or(|splash| !splash.matches_area(area));
                if needs_rebuild {
                    self.splash = Some(Splash::new(area, self.theme));
                }
                let splash_delta = if needs_rebuild {
                    self.splash_elapsed
                } else {
                    delta
                };
                self.splash.as_mut().expect("splash is initialized").apply(
                    buffer,
                    splash_delta,
                    self.splash_elapsed,
                    self.theme,
                );
            }
            Phase::Reveal(reveal_slot) => {
                if reveal_slot
                    .as_ref()
                    .is_none_or(|reveal| reveal.area != area)
                {
                    *reveal_slot = Some(Reveal::new(area, layout, visible_branches, self.theme));
                }
                reveal_slot.as_mut().expect("reveal is initialized").apply(
                    buffer,
                    self.reveal_elapsed,
                    self.theme,
                );
            }
            Phase::Done => {}
        }
    }

    /// Return whether startup still owns the frame.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self.phase, Phase::Done)
    }

    /// Immediately discard the startup frame so the dashboard is shown unmodified.
    pub fn skip(&mut self) {
        self.phase = Phase::Done;
        self.splash = None;
        self.pending_delta = Duration::ZERO;
    }
}
