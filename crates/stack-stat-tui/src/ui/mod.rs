//! Pure terminal layout and widget composition for the current App state.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
};
use stack_stat_core::providers::{GitHubProvider, GitProvider, StatusStoreProvider};

use crate::{
    app::{App, Mode, PointerTarget},
    theme::Theme,
};

/// Tick-based animation state and cell change detection.
pub mod animation;
mod detail;
mod header;
mod help;
mod status_bar;
mod table;
mod verdict;

/// Bold accent-colored title shared by every bordered pane.
fn title(text: impl Into<std::borrow::Cow<'static, str>>, theme: &Theme) -> Span<'static> {
    Span::styled(
        text,
        Style::default()
            .fg(theme.header_title)
            .add_modifier(Modifier::BOLD),
    )
}

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
        Constraint::Length(5),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);
    let body = chunks[1];
    let (table, detail) = if app.selected_branch().is_some() && body.height >= 14 {
        let branch_rows = app.visible_indices().len().min(usize::from(u16::MAX)) as u16;
        let required_table_height = branch_rows.saturating_mul(2).saturating_add(5);
        let table_height = required_table_height
            .min((body.height / 2).max(8))
            .min(body.height.saturating_sub(8));
        (
            Rect {
                height: table_height,
                ..body
            },
            Some(Rect {
                y: body.y.saturating_add(table_height),
                height: body.height.saturating_sub(table_height),
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
        let branch_top = layout.table.y.saturating_add(4);
        if y >= branch_top && y < layout.table.bottom().saturating_sub(1) {
            let row = y.saturating_sub(branch_top) as usize / 2;
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
