//! Compact verdict distribution chart for the stack header.

use ratatui::{Frame, layout::Rect, style::Color};
use stack_stat_core::model::StackSummary;
use tui_piechart::{PieChart, PieSlice, Resolution};

use crate::theme::Theme;

/// Return non-empty verdict segments in clockwise chart order.
#[must_use]
pub(super) fn slices(summary: StackSummary, theme: &Theme) -> Vec<(usize, Color)> {
    [
        (summary.go, theme.success),
        (summary.wait, theme.warning),
        (summary.no_go, theme.error),
        (summary.merged, theme.muted),
        (summary.queued, theme.success),
        (summary.closed, theme.muted),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .collect()
}

/// Render the verdict pie into the supplied chart rectangle.
pub(super) fn render(frame: &mut Frame<'_>, area: Rect, summary: StackSummary, theme: &Theme) {
    let slices = slices(summary, theme)
        .into_iter()
        .map(|(count, color)| PieSlice::new("", count as f64, color))
        .collect();
    frame.render_widget(
        PieChart::new(slices)
            .resolution(Resolution::Braille)
            .show_legend(false)
            .show_percentages(false),
        area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;
    use stack_stat_core::model::StackSummary;

    use super::slices;
    use crate::theme::{ColorDepth, Theme};

    #[test]
    fn slices_keep_verdict_order_and_skip_zero_counts() {
        let theme = Theme::for_depth(ColorDepth::TrueColor);
        let summary = StackSummary {
            go: 2,
            wait: 0,
            no_go: 3,
            merged: 0,
            queued: 1,
            closed: 0,
        };

        assert_eq!(
            slices(summary, &theme),
            vec![(2, theme.success), (3, theme.error), (1, theme.success)]
        );
    }

    #[test]
    fn slices_preserve_proportional_counts() {
        let theme = Theme::for_depth(ColorDepth::TrueColor);
        let summary = StackSummary {
            go: 1,
            wait: 2,
            no_go: 3,
            merged: 4,
            queued: 5,
            closed: 6,
        };

        let slices = slices(summary, &theme);
        assert_eq!(slices.iter().map(|(count, _)| count).sum::<usize>(), 21);
        assert_eq!(
            slices,
            vec![
                (1, theme.success),
                (2, theme.warning),
                (3, theme.error),
                (4, theme.muted),
                (5, theme.success),
                (6, theme.muted),
            ]
        );
    }

    #[test]
    fn slices_return_no_segments_for_an_empty_stack() {
        assert_eq!(
            slices(
                StackSummary::default(),
                &Theme::for_depth(ColorDepth::TrueColor)
            ),
            Vec::<(usize, Color)>::new()
        );
    }
}
