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
    /// Accent color for the header title.
    pub header_title: Color,
    /// Loading placeholder shade.
    pub skeleton: Color,
}

impl Theme {
    /// Construct semantic colors with indexed and 16-color fallbacks.
    #[must_use]
    pub fn for_depth(depth: ColorDepth) -> Self {
        let (
            success,
            error,
            warning,
            muted,
            active_base,
            active_peak,
            selected,
            flash,
            header_title,
            skeleton,
        ) = match depth {
            ColorDepth::TrueColor => (
                Color::Rgb(0x1d, 0xe9, 0xb6),
                Color::Rgb(0xff, 0x5c, 0x6c),
                Color::Rgb(0xff, 0xb4, 0x54),
                Color::Rgb(0x7b, 0x8c, 0xa5),
                Color::Rgb(0x10, 0x2b, 0x3c),
                Color::Rgb(0x16, 0x57, 0x70),
                Color::Rgb(0x13, 0x3a, 0x52),
                Color::Rgb(0x2f, 0x8c, 0xff),
                Color::Rgb(0x2f, 0x8c, 0xff),
                Color::Rgb(0x1a, 0x28, 0x38),
            ),
            ColorDepth::Ansi256 => (
                Color::Indexed(50),
                Color::Indexed(203),
                Color::Indexed(215),
                Color::Indexed(103),
                Color::Indexed(23),
                Color::Indexed(31),
                Color::Indexed(24),
                Color::Indexed(33),
                Color::Indexed(33),
                Color::Indexed(236),
            ),
            ColorDepth::Basic16 => (
                Color::Cyan,
                Color::Red,
                Color::Yellow,
                Color::Gray,
                Color::Blue,
                Color::Cyan,
                Color::Blue,
                Color::Blue,
                Color::Blue,
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
            header_title,
            skeleton,
        }
    }
}
