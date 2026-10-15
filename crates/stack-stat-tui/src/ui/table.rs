//! Compact stack rows with responsive status columns.

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use stack_stat_core::{
    model::{PrLifecycle, RebaseNeed, StackMode, StackStatus, Verdict},
    providers::{GitHubProvider, GitProvider, StatusStoreProvider},
};

use crate::{
    app::App,
    theme::{ColorDepth, Theme},
    ui::animation::Column,
};

mod labels;
use labels::{
    check_label, lifecycle_label, local_label, rebase_label, review_label, thread_label,
    worktree_marker,
};

pub(super) fn view_start<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    app: &App<G, H, S>,
    area: Rect,
) -> usize {
    let branch_capacity = usize::from(area.height.saturating_sub(5) / 2).max(1);
    app.selected_row_index()
        .saturating_add(1)
        .saturating_sub(branch_capacity)
}

pub(super) fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    if app.state().stack_status == StackStatus::NotInStack {
        let guidance = super::default_branch_name(app.state()).map_or_else(
            || {
                "No attached branch\nCheck out a branch to view its dashboard,\nwith or without a PR."
                    .to_owned()
            },
            |branch| {
                format!(
                    "{branch} is the repository default branch\nCheck out a feature branch to view its dashboard,\nwith or without a PR."
                )
            },
        );
        frame.render_widget(
            Paragraph::new(guidance).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(super::title(" Stack ", theme)),
            ),
            area,
        );
        return;
    }
    let full_reviews = area.width >= 100;
    let full_rebase = area.width >= 100;
    let full_local = area.width >= 80;
    // The Rebase column only appears when a layer needs rebase attention.
    let show_rebase = app
        .state()
        .branches
        .iter()
        .any(|branch| branch.rebase != RebaseNeed::None);
    // The Local column only appears once local checks are configured or reported for some branch.
    let show_local = app
        .state()
        .branches
        .iter()
        .any(|branch| !branch.local_checks.is_empty());
    let mut widths = vec![
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Min(10),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Length(if full_reviews { 17 } else { 5 }),
        Constraint::Length(7),
    ];
    let mut header = vec![
        "",
        "",
        "Branch",
        "PR",
        "State",
        "Checks",
        if full_reviews { "Reviews" } else { "Rev" },
        if full_reviews { "Threads" } else { "Thr" },
    ];
    if show_rebase {
        // Full rows name the trunk ("↻ develop"); fit it within a bounded column.
        let rebase_width = if full_rebase {
            (app.state().trunk.chars().count() + 2).clamp(7, 20) as u16
        } else {
            7
        };
        widths.push(Constraint::Length(rebase_width));
        header.push(if full_rebase { "Rebase" } else { "Rb" });
    }
    if show_local {
        widths.push(Constraint::Length(if full_local { 22 } else { 4 }));
        header.push(if full_local { "Local" } else { "L" });
    }
    let header = Row::new(header).style(
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    );
    let branch_capacity = usize::from(area.height.saturating_sub(5) / 2).max(1);
    let trunk = if app.state().trunk.is_empty() {
        "trunk"
    } else {
        app.state().trunk.as_str()
    };
    let mut trunk_cells = vec![
        Cell::from(""),
        Cell::from(vec![Line::from("○"), Line::from("│")]),
        Cell::from(Span::styled(trunk, Style::default().fg(theme.muted))),
    ];
    trunk_cells.resize_with(widths.len(), || Cell::from(""));
    let mut rows = vec![
        Row::new(trunk_cells)
            .height(2)
            .style(Style::default().fg(theme.muted)),
    ];
    let visible = app.visible_indices();
    let start = view_start(app, area);
    let visible_count = visible.len();
    rows.extend(
        visible
            .into_iter()
            .skip(start)
            .take(branch_capacity)
            .enumerate()
            .map(|(offset, index)| {
                let branch = &app.state().branches[index];
                let selected = app.selected_branch() == Some(branch.name.as_str());
                let no_pr = branch.pr_number.is_none() && branch.pr.is_none();
                let verdict = branch.verdict();
                let (glyph, verdict_color) = super::verdict::style(verdict, theme);
                let (lifecycle, lifecycle_color) = lifecycle_label(branch, theme);
                let checks = check_label(branch, app.animation().spinner_char());
                let reviews = review_label(
                    branch,
                    app.show_bots(),
                    app.reviews_expanded(),
                    full_reviews,
                );
                let threads = thread_label(branch);
                let rebase = rebase_label(branch, &app.state().trunk, full_rebase);
                let local = local_label(branch, full_local);
                let branch_name = branch.name.as_str();
                let title = if no_pr {
                    "no PR yet"
                } else {
                    branch.pr.as_ref().map_or("", |pr| pr.title.as_str())
                };
                // A merged or closed layer is history: every column is dimmed.
                let dim = matches!(branch.lifecycle, PrLifecycle::Merged | PrLifecycle::Closed);
                let style = |column: Column, foreground| {
                    let style = Style::default().fg(if dim { theme.muted } else { foreground });
                    if app.animation().is_flashing_for(&branch.name, column) {
                        style.bg(theme.flash)
                    } else {
                        style
                    }
                };
                let (marker, sync_marker) = worktree_marker(branch);
                let connector = if start + offset + 1 == visible_count {
                    " "
                } else {
                    "│"
                };
                let branch_color = match verdict {
                    Verdict::Merged | Verdict::Closed => ratatui::style::Color::Reset,
                    _ => verdict_color,
                };
                let mut cells = vec![
                    Cell::from(vec![
                        Line::from(vec![
                            Span::raw(marker),
                            Span::styled(sync_marker, Style::default().fg(theme.muted)),
                        ]),
                        Line::from(""),
                    ]),
                    Cell::from(vec![
                        Line::styled(glyph, Style::default().fg(verdict_color)),
                        Line::styled(connector, Style::default().fg(verdict_color)),
                    ]),
                    Cell::from(vec![
                        Line::from(Span::styled(
                            branch_name,
                            if dim {
                                style(Column::Branch, branch_color)
                            } else {
                                style(Column::Branch, branch_color).add_modifier(Modifier::BOLD)
                            },
                        )),
                        Line::styled(
                            if title.is_empty() {
                                String::new()
                            } else {
                                format!("  {title}")
                            },
                            Style::default().fg(theme.muted),
                        ),
                    ]),
                    Cell::from(Span::styled(
                        branch
                            .pr_number
                            .map_or_else(String::new, |number| format!("#{number}")),
                        style(Column::Pr, ratatui::style::Color::Reset),
                    )),
                    Cell::from(Span::styled(
                        lifecycle,
                        style(Column::State, lifecycle_color),
                    )),
                    Cell::from(Span::styled(
                        checks.0,
                        style(Column::Checks, checks.1.color(theme)),
                    )),
                    Cell::from(Span::styled(
                        reviews.0,
                        style(Column::Reviews, reviews.1.color(theme)),
                    )),
                    Cell::from(Span::styled(
                        threads.0,
                        style(Column::Threads, threads.1.color(theme)),
                    )),
                ];
                if show_rebase {
                    cells.push(Cell::from(Span::styled(
                        rebase.0,
                        style(Column::Rebase, rebase.1.color(theme)),
                    )));
                }
                if show_local {
                    cells.push(Cell::from(Span::styled(
                        local.0,
                        style(Column::Local, local.1.color(theme)),
                    )));
                }
                let bg = if selected {
                    app.animation()
                        .pulse_color(theme.active_base, theme.active_peak, 20)
                } else {
                    ratatui::style::Color::Reset
                };
                let mut row_style = Style::default().bg(bg);
                if selected
                    && theme.depth == ColorDepth::Basic16
                    && app.animation().phase(20) >= 0.5
                {
                    row_style = row_style.add_modifier(Modifier::BOLD);
                }
                Row::new(cells).height(2).style(row_style)
            }),
    );
    if app.state().stack_status == StackStatus::Loading {
        let shimmer = |column| app.animation().skeleton_char(column).to_string();
        let mut skeleton: Vec<String> = vec![" ".into()];
        skeleton.extend((0..widths.len() - 1).map(shimmer));
        rows.push(
            Row::new(skeleton)
                .height(2)
                .style(Style::default().fg(theme.skeleton)),
        );
    }
    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(super::title(
                    if app.state().mode == StackMode::Single {
                        " Branch "
                    } else {
                        " Stack "
                    },
                    theme,
                ))
                .border_style(Style::default().fg(theme.border)),
        );
    frame.render_widget(table, area);
}
