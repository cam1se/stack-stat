//! Selected branch detail pane with complete per-check and reviewer evidence.

use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use stack_stat_core::{
    model::RebaseNeed,
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{app::App, theme::Theme, wrap};

mod evidence;
use evidence::{
    check_counts, check_group, check_line, check_title, lifecycle_word, local_check_group,
    local_check_line, local_sync_text, review_group, review_headline, review_line,
};

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    let Some(branch) = app.selected_branch().and_then(|name| {
        app.state()
            .branches
            .iter()
            .find(|branch| branch.name == name)
    }) else {
        return;
    };
    let no_pr = branch.pr_number.is_none() && branch.pr.is_none();

    let (glyph, verdict_color) = super::verdict::style(branch.verdict(), theme);
    let lifecycle = lifecycle_word(branch);
    let content_width = usize::from(area.width.saturating_sub(2));
    let mut header = Vec::new();
    if let Some(number) = branch.pr_number {
        header.push(Span::raw(format!("PR #{number} · ")));
    }
    header.extend([
        Span::styled(glyph, Style::default().fg(verdict_color)),
        Span::raw(" · "),
        Span::styled(
            lifecycle,
            Style::default().fg(match lifecycle {
                "Open" | "Queued" => theme.success,
                "Draft" => theme.error,
                _ => theme.muted,
            }),
        ),
    ]);
    if branch.is_current {
        header.extend([
            Span::raw("   * current worktree · "),
            Span::styled(
                local_sync_text(branch.local_sync),
                Style::default().fg(theme.muted),
            ),
        ]);
    }
    let mut lines = vec![Line::from(header)];
    // The PR title is the headline with the branch dimmed and indented beneath it; without a
    // title the branch stays the headline.
    match branch.pr.as_ref().filter(|pr| !pr.title.is_empty()) {
        Some(pr) => {
            lines.extend(
                wrap::wrap(&pr.title, content_width)
                    .into_iter()
                    .map(|line| Line::styled(line, Style::default().add_modifier(Modifier::BOLD))),
            );
            lines.extend(
                wrap::wrap(&branch.name, content_width.saturating_sub(2))
                    .into_iter()
                    .map(|line| {
                        Line::styled(format!("  {line}"), Style::default().fg(theme.muted))
                    }),
            );
        }
        None => lines.extend(
            wrap::wrap(&branch.name, content_width)
                .into_iter()
                .map(|line| Line::styled(line, Style::default().add_modifier(Modifier::BOLD))),
        ),
    }
    match branch.rebase {
        RebaseNeed::None => {}
        RebaseNeed::Parent => {
            let parent = app
                .state()
                .branches
                .iter()
                .position(|candidate| candidate.name == branch.name)
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| app.state().branches.get(index))
                .map_or("layer below", |parent| parent.name.as_str());
            lines.push(Line::styled(
                format!("Rebase needed: onto {parent}"),
                Style::default().fg(theme.error),
            ));
        }
        RebaseNeed::Stacked => {
            lines.push(Line::styled(
                "Rebase needed: a layer below needs a rebase",
                Style::default().fg(theme.error),
            ));
        }
        RebaseNeed::Trunk => {
            let blocking = branch.trunk_rebase_gates;
            lines.push(Line::styled(
                format!(
                    "Behind {}: rebase onto trunk{}",
                    app.state().trunk,
                    if blocking { "" } else { " (not blocking)" }
                ),
                Style::default().fg(if blocking { theme.error } else { theme.muted }),
            ));
        }
    }

    if !no_pr {
        // A no-PR row has no checks or reviews; the local-checks gap below separates it.
        lines.push(Line::from(""));
    }
    if let Some(pr) = &branch.pr {
        let counts = check_counts(&pr.checks);
        lines.push(Line::from(check_title(counts)));
        if pr.checks.is_empty() {
            lines.push(Line::from("  — none reported"));
        } else {
            let mut checks: Vec<_> = pr.checks.iter().collect();
            checks.sort_by_key(|check| check_group(check));
            for check in checks {
                lines.push(check_line(check, area.width, app, theme));
            }
        }
    } else if !no_pr {
        lines.push(Line::from("Checks:"));
        lines.push(Line::from("  — no PR details"));
    }

    if !no_pr {
        lines.push(Line::from(""));
        lines.push(Line::from("Reviews:"));
        if let Some(pr) = &branch.pr {
            let mut reviews: Vec<_> = pr
                .reviews
                .iter()
                .filter(|review| app.show_bots() || !review.is_bot)
                .collect();
            let (headline, headline_color) = review_headline(&reviews, pr.review_decision, theme);
            lines.push(Line::styled(
                format!("  {headline}"),
                Style::default().fg(headline_color),
            ));
            reviews.sort_by_key(|review| review_group(review));
            for review in reviews {
                lines.push(review_line(review, theme));
            }
        } else {
            lines.push(Line::from("  — no PR details"));
        }

        lines.push(Line::from(""));
        if let Some(pr) = &branch.pr {
            let color = if pr.unresolved_threads == 0 {
                theme.success
            } else if pr.threads_gate {
                theme.error
            } else {
                theme.warning
            };
            lines.push(Line::styled(
                format!("Threads: Unresolved threads: {}", pr.unresolved_threads),
                Style::default().fg(color),
            ));
        } else {
            lines.push(Line::styled(
                "Threads: — no PR details",
                Style::default().fg(theme.muted),
            ));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from("Local checks:"));
    if branch.local_checks.is_empty() {
        lines.push(Line::from("  — none declared or reported"));
    } else {
        let mut checks: Vec<_> = branch.local_checks.iter().collect();
        checks.sort_by_key(|check| local_check_group(check));
        for check in checks {
            lines.push(local_check_line(check, theme));
        }
    }
    render_scrolled(frame, area, app, theme, lines);
}

fn render_scrolled<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
    lines: Vec<Line<'static>>,
) {
    let content_height = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let max_scroll = total.saturating_sub(content_height);
    let scroll = app.detail_scroll().min(max_scroll);
    let visible: Vec<_> = lines
        .into_iter()
        .skip(scroll)
        .take(content_height)
        .collect();
    frame.render_widget(
        Paragraph::new(visible).block(
            Block::default()
                .borders(Borders::ALL)
                .title(super::title(" Details ", theme))
                .border_style(Style::default().fg(theme.border)),
        ),
        area,
    );
    if max_scroll > 0 && content_height > 0 {
        let mut state = ScrollbarState::new(max_scroll)
            .position(scroll)
            .viewport_content_length(content_height);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::default().fg(theme.header_title)),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut state,
        );
    }
}
