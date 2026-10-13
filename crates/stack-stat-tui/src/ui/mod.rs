//! Pure terminal layout and widget composition for the current App state.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};
use stack_stat_core::providers::{GitHubProvider, GitProvider, StatusStoreProvider};

use crate::{
    app::{App, Mode, PointerTarget},
    theme::Theme,
};

mod detail;
mod header;
mod help;
mod status_bar;
mod table;

/// Rectangles shared by pure drawing and mouse hit testing.
#[derive(Clone, Copy, Debug)]
pub struct UiRegions {
    /// Stack identity at the top.
    pub header: Rect,
    /// Branch status table.
    pub table: Rect,
    /// Optional selected-branch details.
    pub detail: Option<Rect>,
    /// Footer hints and feedback.
    pub status: Rect,
}

/// Calculate pane geometry without mutating App state.
#[must_use]
pub fn regions<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    area: Rect,
    app: &App<G, H, S>,
) -> UiRegions {
    let chunks = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    let body = chunks[1];
    let (table, detail) =
        if app.detail_open() && app.selected_branch().is_some() && body.height >= 10 {
            let table_min = (app.visible_indices().len() as u16 + 3).max(4);
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
    UiRegions {
        header: chunks[0],
        table,
        detail,
        status: chunks[2],
    }
}

/// Determine the pane or visible row under a mouse coordinate.
#[must_use]
pub fn hit_test<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider>(
    area: Rect,
    app: &App<G, H, S>,
    x: u16,
    y: u16,
) -> PointerTarget {
    if app.mode() == Mode::Help {
        return PointerTarget::Outside;
    }
    let layout = regions(area, app);
    if layout.detail.is_some_and(|detail| contains(detail, x, y)) {
        return PointerTarget::Detail;
    }
    if contains(layout.table, x, y) {
        let row = y.saturating_sub(layout.table.y + 2) as usize;
        if y >= layout.table.y + 2 && y < layout.table.bottom().saturating_sub(1) {
            let index = table::view_start(app, layout.table) + row;
            if index < app.visible_indices().len() {
                return PointerTarget::TableRow(index);
            }
        }
        return PointerTarget::Table;
    }
    PointerTarget::Outside
}

fn contains(region: Rect, x: u16, y: u16) -> bool {
    x >= region.x && x < region.right() && y >= region.y && y < region.bottom()
}

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
    let layout = regions(area, app);
    header::render(frame, layout.header, app, theme);
    table::render(frame, layout.table, app, theme);
    if let Some(detail) = layout.detail {
        detail::render(frame, detail, app, theme);
    }
    status_bar::render(frame, layout.status, app, theme);
    if app.mode() == Mode::Help {
        help::render(frame, area, theme);
    }
}
