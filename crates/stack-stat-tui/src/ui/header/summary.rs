//! Merge candidate and verdict summaries for the dashboard header.

use ratatui::{
    style::Style,
    text::{Line, Span},
};
use stack_stat_core::model::{Issue, StackState, StackSummary, Verdict};

use crate::theme::Theme;

/// The next layer to merge as a headline plus one indented bullet per issue.
pub(super) fn next_to_merge_lines(
    state: &StackState,
    theme: &Theme,
) -> (Line<'static>, Vec<Line<'static>>) {
    let Some(next) = state.next_to_merge() else {
        if let Some(branch) = state
            .branches
            .iter()
            .find(|branch| branch.pr_number.is_none() && branch.pr.is_none())
        {
            let bullets = branch
                .issues()
                .into_iter()
                .filter(|issue| !matches!(issue, Issue::NoPr))
                .map(|issue| issue_line(issue, theme))
                .collect();
            return (
                Line::styled(
                    "no PR yet — create a PR when this branch is ready",
                    Style::default().fg(theme.warning),
                ),
                bullets,
            );
        }
        return (
            Line::styled("Nothing left to merge", Style::default().fg(theme.muted)),
            Vec::new(),
        );
    };
    let (glyph, verdict_color) = super::super::verdict::style(next.verdict(), theme);
    let number = next
        .pr_number
        .map_or_else(String::new, |number| format!("#{number} "));
    let issues = next.issues();
    let mut head = vec![
        Span::styled(glyph, Style::default().fg(verdict_color)),
        Span::styled(" Next: ", Style::default().fg(theme.muted)),
        Span::styled(number, Style::default().fg(theme.accent)),
        Span::raw(next.name.clone()),
    ];
    if issues.is_empty() {
        head.push(Span::raw(" — "));
        head.push(Span::styled(
            "ready to merge",
            Style::default().fg(theme.success),
        ));
    }
    let bullets = issues
        .into_iter()
        .map(|issue| issue_line(issue, theme))
        .collect();
    (Line::from(head), bullets)
}

pub(super) fn blocker_line(state: &StackState, theme: &Theme) -> Option<Line<'static>> {
    let summary = state.blocker_summary();
    let categories = [
        ("checks", summary.checks, theme.error),
        ("reviews", summary.reviews, theme.warning),
        ("threads", summary.threads, theme.error),
        ("rebase", summary.rebase, theme.error),
        ("local", summary.local, theme.warning),
        ("draft", summary.draft, theme.error),
    ];
    let mut spans = vec![Span::styled(
        "Blocked by: ",
        Style::default().fg(theme.muted),
    )];
    let mut has_category = false;
    for (name, count, color) in categories {
        if count == 0 {
            continue;
        }
        if has_category {
            spans.push(Span::raw(" · "));
        }
        spans.push(Span::styled(
            format!("{name} {count}"),
            Style::default().fg(color),
        ));
        has_category = true;
    }
    has_category.then(|| Line::from(spans))
}

pub(super) fn summary_line(summary: StackSummary, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (verdict, count, always_visible) in [
        (Verdict::Go, summary.go, true),
        (Verdict::Wait, summary.wait, true),
        (Verdict::NoGo, summary.no_go, true),
        (Verdict::Merged, summary.merged, false),
        (Verdict::Queued, summary.queued, false),
        (Verdict::Closed, summary.closed, false),
    ] {
        if !always_visible && count == 0 {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::raw("   "));
        }
        let (glyph, color) = super::super::verdict::style(verdict, theme);
        spans.push(Span::styled(
            format!("{glyph} {count}"),
            Style::default().fg(color),
        ));
    }
    Line::from(spans)
}

fn issue_line(issue: Issue, theme: &Theme) -> Line<'static> {
    let color = if issue.is_blocking() {
        theme.error
    } else {
        theme.warning
    };
    Line::styled(
        format!("  - {}", super::super::issues::describe(issue)),
        Style::default().fg(color),
    )
}
