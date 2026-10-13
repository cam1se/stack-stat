//! Compact stack rows with responsive status columns.

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use stack_stat_core::{
    model::{
        BranchState, CheckHealth, LocalCheckState, PrLifecycle, ReviewDecision, ReviewState,
        ReviewSummary, StackStatus,
    },
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{
    app::App,
    theme::{ColorDepth, Theme},
    ui::animation::Column,
};

pub(super) fn view_start<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    app: &App<G, H, S>,
    area: Rect,
) -> usize {
    let capacity = area.height.saturating_sub(3).max(1) as usize;
    app.selected_row_index()
        .saturating_add(1)
        .saturating_sub(capacity)
}

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    if app.state().stack_status == StackStatus::NotInStack {
        frame.render_widget(
            Paragraph::new(
                "Not in a stack\nSwitch to a stacked branch or run gh stack init <branch>.",
            )
            .block(Block::default().borders(Borders::ALL).title(" Stack ")),
            area,
        );
        return;
    }
    let full_reviews = area.width >= 100;
    let full_local = area.width >= 80;
    let widths = [
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Length(if full_reviews { 17 } else { 5 }),
        Constraint::Length(if full_local { 22 } else { 4 }),
    ];
    let header = Row::new([
        "",
        "Branch",
        "PR",
        "State",
        "Checks",
        if full_reviews { "Reviews" } else { "Rev" },
        if full_local { "Local" } else { "L" },
    ])
    .style(
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    );
    let capacity = area.height.saturating_sub(3) as usize;
    let mut rows: Vec<Row<'_>> = app
        .visible_indices()
        .into_iter()
        .skip(view_start(app, area))
        .take(capacity)
        .map(|index| {
            let branch = &app.state().branches[index];
            let selected = app.selected_branch() == Some(branch.name.as_str());
            let (state, state_color) = state_label(branch, theme);
            let checks = check_label(branch, app.animation().spinner_char());
            let reviews = review_label(
                branch,
                app.show_bots(),
                app.reviews_expanded(),
                full_reviews,
            );
            let local = local_label(branch, full_local);
            let branch_name = format!(
                "{}{}{}",
                branch.name,
                if branch.is_current { " ←wt" } else { "" },
                if branch.needs_rebase { " ⚠" } else { "" }
            );
            let style = |column: Column, foreground| {
                let style = Style::default().fg(foreground);
                if app.animation().is_flashing_for(&branch.name, column) {
                    style.bg(theme.flash)
                } else {
                    style
                }
            };
            let cells = vec![
                Cell::from(if selected { "►" } else { " " }),
                Cell::from(Span::styled(
                    branch_name,
                    style(Column::Branch, ratatui::style::Color::Reset),
                )),
                Cell::from(Span::styled(
                    branch
                        .pr_number
                        .map_or_else(|| "—".into(), |number| format!("#{number}")),
                    style(Column::Pr, ratatui::style::Color::Reset),
                )),
                Cell::from(Span::styled(state, style(Column::State, state_color))),
                Cell::from(Span::styled(
                    checks.0,
                    style(Column::Checks, checks.1.color(theme)),
                )),
                Cell::from(Span::styled(
                    reviews.0,
                    style(Column::Reviews, reviews.1.color(theme)),
                )),
                Cell::from(Span::styled(
                    local.0,
                    style(Column::Local, local.1.color(theme)),
                )),
            ];
            let bg = if branch.is_current {
                app.animation()
                    .pulse_color(theme.active_base, theme.active_peak, 20)
            } else if selected {
                theme.selected
            } else {
                ratatui::style::Color::Reset
            };
            let mut row_style = Style::default().bg(bg);
            if branch.is_current
                && theme.depth == ColorDepth::Basic16
                && app.animation().phase(20) >= 0.5
            {
                row_style = row_style.add_modifier(Modifier::BOLD);
            }
            Row::new(cells).style(row_style)
        })
        .collect();
    if app.state().stack_status == StackStatus::Loading {
        let shimmer = |column| app.animation().skeleton_char(column).to_string();
        rows.push(
            Row::new([
                " ".into(),
                shimmer(0),
                shimmer(1),
                shimmer(2),
                shimmer(3),
                shimmer(4),
                shimmer(5),
            ])
            .style(Style::default().fg(theme.skeleton)),
        );
    }
    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Stack ")
                .border_style(Style::default().fg(theme.muted)),
        );
    frame.render_widget(table, area);
}

fn state_label(branch: &BranchState, theme: &Theme) -> (String, ratatui::style::Color) {
    match branch.lifecycle {
        PrLifecycle::Merged => ("Merged".into(), theme.success),
        PrLifecycle::Queued => ("Queued".into(), theme.warning),
        PrLifecycle::Open => match branch.pr.as_ref() {
            Some(pr) if pr.is_draft => ("Draft".into(), theme.muted),
            Some(_) => ("Ready".into(), theme.success),
            None if branch.pr_number.is_some() => ("…".into(), theme.warning),
            None => ("—".into(), theme.muted),
        },
    }
}

#[derive(Clone, Copy)]
enum Health {
    Success,
    Failure,
    Running,
    Muted,
}
impl Health {
    fn color(self, theme: &Theme) -> ratatui::style::Color {
        match self {
            Self::Success => theme.success,
            Self::Failure => theme.error,
            Self::Running => theme.warning,
            Self::Muted => theme.muted,
        }
    }
}

fn check_label(branch: &BranchState, spinner: char) -> (String, Health) {
    let Some(pr) = &branch.pr else {
        return ("—".into(), Health::Muted);
    };
    let summary = pr.check_summary();
    if summary.total == 0 {
        return ("—".into(), Health::Muted);
    }
    let (icon, health) = match summary.worst {
        CheckHealth::Failure => ('❌', Health::Failure),
        CheckHealth::Running => (spinner, Health::Running),
        CheckHealth::Success => ('✅', Health::Success),
        CheckHealth::Skipped | CheckHealth::None => ('░', Health::Muted),
    };
    (
        format!("{icon} {}/{}", summary.passed, summary.total),
        health,
    )
}

fn review_label(
    branch: &BranchState,
    show_bots: bool,
    expanded: bool,
    full: bool,
) -> (String, Health) {
    let Some(pr) = &branch.pr else {
        return ("—".into(), Health::Muted);
    };
    let summary = pr.review_summary(show_bots);
    let summary = if pr.reviews.is_empty() {
        match pr.review_decision {
            ReviewDecision::Approved => ReviewSummary::Approved,
            ReviewDecision::ChangesRequested => ReviewSummary::ChangesRequested,
            ReviewDecision::ReviewRequired => ReviewSummary::Pending,
            ReviewDecision::None => summary,
        }
    } else {
        summary
    };
    let (icon, label, health) = match summary {
        ReviewSummary::Approved => ("✅", "approved", Health::Success),
        ReviewSummary::ChangesRequested => ("🔄", "changes", Health::Failure),
        ReviewSummary::Pending => ("⏳", "pending", Health::Running),
        ReviewSummary::None => ("—", "", Health::Muted),
    };
    if !full {
        return (icon.into(), health);
    }
    if expanded {
        let names: Vec<_> = pr
            .reviews
            .iter()
            .filter(|review| show_bots || !review.is_bot)
            .map(|review| {
                let icon = match review.state {
                    ReviewState::Approved => "✅",
                    ReviewState::ChangesRequested => "🔄",
                    ReviewState::Pending => "⏳",
                    _ => "·",
                };
                format!("{icon}{}", review.author)
            })
            .collect();
        if !names.is_empty() {
            return (names.join(" "), health);
        }
    }
    (
        if label.is_empty() {
            icon.into()
        } else {
            format!("{icon} {label}")
        },
        health,
    )
}

fn local_label(branch: &BranchState, full: bool) -> (String, Health) {
    if branch.local_checks.is_empty() {
        return ("—".into(), Health::Muted);
    }
    if !full {
        let health = if branch
            .local_checks
            .iter()
            .any(|check| check.state == LocalCheckState::Fail && !check.is_stale)
        {
            Health::Failure
        } else if branch
            .local_checks
            .iter()
            .any(|check| check.is_stale || check.state == LocalCheckState::Missing)
        {
            Health::Running
        } else if branch
            .local_checks
            .iter()
            .any(|check| check.state == LocalCheckState::Pass)
        {
            Health::Success
        } else {
            Health::Muted
        };
        let icon = match health {
            Health::Failure => "❌",
            Health::Running => "⏳",
            Health::Success => "✅",
            Health::Muted => "░░",
        };
        return (icon.into(), health);
    }
    let items: Vec<_> = branch
        .local_checks
        .iter()
        .map(|check| {
            let icon = if check.is_stale {
                "⏳"
            } else {
                match check.state {
                    LocalCheckState::Pass => "✅",
                    LocalCheckState::Fail => "❌",
                    LocalCheckState::Skip => "░░",
                    LocalCheckState::Missing => "⏳",
                }
            };
            format!("{icon} {}", check.name)
        })
        .collect();
    let health = if branch
        .local_checks
        .iter()
        .any(|check| check.state == LocalCheckState::Fail && !check.is_stale)
    {
        Health::Failure
    } else if branch
        .local_checks
        .iter()
        .any(|check| check.is_stale || check.state == LocalCheckState::Missing)
    {
        Health::Running
    } else {
        Health::Success
    };
    (items.join(" "), health)
}
