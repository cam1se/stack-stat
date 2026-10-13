//! Semantic terminal colors with truecolor, 256-color, and basic fallbacks.

use ratatui::style::Color;

/// Available terminal color fidelity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDepth {
    /// Basic ANSI palette.
    Basic16,
    /// Indexed 256-color palette.
    Ansi256,
    /// Full 24-bit RGB palette.
    TrueColor,
}

impl ColorDepth {
    /// Detect depth from passed environment values without reading globals.
    #[must_use]
    pub fn detect_from(colorterm: Option<&str>, term: Option<&str>) -> Self {
        if colorterm.is_some_and(|value| value.contains("truecolor") || value.contains("24bit")) {
            Self::TrueColor
        } else if colorterm.is_some_and(|value| value.contains("256color"))
            || term.is_some_and(|value| value.contains("256color"))
        {
            Self::Ansi256
        } else {
            Self::Basic16
        }
    }

    /// Detect the active terminal from COLORTERM and TERM.
    #[must_use]
    pub fn detect() -> Self {
        let colorterm = std::env::var("COLORTERM").ok();
        let term = std::env::var("TERM").ok();
        Self::detect_from(colorterm.as_deref(), term.as_deref())
    }
}

/// Status palette selected for one terminal color depth.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Detected color depth.
    pub depth: ColorDepth,
    /// Passing checks and approvals.
    pub success: Color,
    /// Failing checks and requested changes.
    pub error: Color,
    /// Running checks and pending reviews.
    pub warning: Color,
    /// Draft, skipped, and stale values.
    pub muted: Color,
    /// Base active-worktree background.
    pub active_base: Color,
    /// Peak worktree highlight used by animation.
    pub active_peak: Color,
    /// Selected-row background.
    pub selected: Color,
    /// Flash highlight on changed data.
    pub flash: Color,
    /// Loading placeholder shade.
    pub skeleton: Color,
}

impl Theme {
    /// Construct semantic colors with indexed and 16-color fallbacks.
    #[must_use]
    pub fn for_depth(depth: ColorDepth) -> Self {
        let (success, error, warning, muted, active_base, active_peak, selected, flash, skeleton) =
            match depth {
                ColorDepth::TrueColor => (
                    Color::Rgb(0x3f, 0xb9, 0x50),
                    Color::Rgb(0xf8, 0x51, 0x49),
                    Color::Rgb(0xd2, 0x99, 0x22),
                    Color::Rgb(0x8b, 0x94, 0x9e),
                    Color::Rgb(0x1a, 0x28, 0x40),
                    Color::Rgb(0x2a, 0x3f, 0x5f),
                    Color::Rgb(0x2d, 0x33, 0x3b),
                    Color::Rgb(0x58, 0xa6, 0xff),
                    Color::Rgb(0x30, 0x36, 0x3d),
                ),
                ColorDepth::Ansi256 => (
                    Color::Indexed(34),
                    Color::Indexed(196),
                    Color::Indexed(178),
                    Color::Indexed(245),
                    Color::Indexed(236),
                    Color::Indexed(238),
                    Color::Indexed(237),
                    Color::Indexed(75),
                    Color::Indexed(240),
                ),
                ColorDepth::Basic16 => (
                    Color::Green,
                    Color::Red,
                    Color::Yellow,
                    Color::Gray,
                    Color::Blue,
                    Color::Blue,
                    Color::DarkGray,
                    Color::Cyan,
                    Color::DarkGray,
                ),
            };
        Self {
            depth,
            success,
            error,
            warning,
            muted,
            active_base,
            active_peak,
            selected,
            flash,
            skeleton,
        }
    }
}
