//! Pure terminal layout and widget composition for the current App state.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};
use stack_stat_core::providers::{GitHubProvider, GitProvider, StatusStoreProvider};

use crate::{
    app::{App, Mode},
    theme::Theme,
};

mod detail;
mod header;
mod help;
mod status_bar;
mod table;

/// Render header, branch table, optional details, footer, and modal help.
pub fn render<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    frame: &mut Frame<'_>,
    app: &App<G, H, S>,
    theme: &Theme,
) {
    let area = frame.area();
    if area.height == 0 || area.width == 0 {
        return;
    }
    let regions = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    header::render(frame, regions[0], app, theme);
    let body = regions[1];
    let (table_region, detail_region) =
        if app.detail_open() && app.selected_branch().is_some() && body.height >= 10 {
            let table_min = (app.state().branches.len() as u16 + 3).max(4);
            let detail_height = body
                .height
                .saturating_sub(table_min)
                .clamp(8, 18)
                .min(body.height.saturating_sub(4));
            let table_height = body.height - detail_height;
            (
                Rect {
                    height: table_height,
                    ..body
                },
                Some(Rect {
                    y: body.y + table_height,
                    height: detail_height,
                    ..body
                }),
            )
        } else {
            (body, None)
        };
    table::render(frame, table_region, app, theme);
    if let Some(region) = detail_region {
        detail::render(frame, region, app, theme);
    }
    status_bar::render(frame, regions[2], app, theme);
    if app.mode() == Mode::Help {
        help::render(frame, area, theme);
    }
}
