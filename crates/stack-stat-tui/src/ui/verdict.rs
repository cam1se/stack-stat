//! Shared presentation of go/no-go verdicts so rows and details agree.

use ratatui::style::Color;
use stack_stat_core::model::Verdict;

use crate::theme::Theme;

/// Glyph and color for a verdict. Glyphs differ per verdict so state stays distinguishable
/// without color.
#[must_use]
pub(super) fn style(verdict: Verdict, theme: &Theme) -> (&'static str, Color) {
    match verdict {
        Verdict::Go => ("✔", theme.success),
        Verdict::Wait => ("…", theme.warning),
        Verdict::NoGo => ("✖", theme.error),
        Verdict::Merged => ("✓", theme.muted),
        Verdict::Queued => ("»", theme.success),
    }
}
