mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui, wrap,
};
use support::{PrBuilder, TestScenario, find_row};

fn draw(app: &support::FakeApp, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::TrueColor)))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn detail_rows(buffer: &ratatui::buffer::Buffer, detail: Rect) -> Vec<String> {
    ((detail.y + 1)..detail.bottom().saturating_sub(1))
        .map(|y| {
            ((detail.x + 1)..detail.right().saturating_sub(1))
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        })
        .collect()
}

#[tokio::test]
async fn detail_wraps_long_branch_name_and_title_without_clipping() {
    let branch = "feature/replace-the-detail-header-with-a-fully-wrapped-branch-name";
    let title = "Make the selected pull request detail header preserve every readable word";
    let (mut app, _) = TestScenario::builder()
        .stack("main", &[branch])
        .pr(branch, PrBuilder::new(1833).title(title))
        .current_branch(branch)
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let area = Rect::new(0, 0, 54, 48);
    let detail = ui::regions(area, &app)
        .detail
        .expect("selected branch has details");
    let content_width = usize::from(detail.width.saturating_sub(2));
    let buffer = draw(&app, area.width, area.height);
    let rows = detail_rows(&buffer, detail);
    let branch_lines = wrap::wrap(branch, content_width);
    let title_lines = wrap::wrap(title, content_width.saturating_sub(2));

    assert!(rows[0].contains("PR #1833"));
    assert_eq!(
        rows[1..1 + branch_lines.len()]
            .iter()
            .map(|line| line.trim_end())
            .collect::<String>(),
        branch
    );
    assert!(
        rows[1 + branch_lines.len()..1 + branch_lines.len() + title_lines.len()]
            .iter()
            .all(|line| line.starts_with("  "))
    );
    assert_eq!(
        rows[1 + branch_lines.len()..1 + branch_lines.len() + title_lines.len()]
            .iter()
            .map(|line| line.trim().to_owned())
            .collect::<Vec<_>>()
            .join(" "),
        title
    );
    for y in (detail.y + 1)..detail.bottom().saturating_sub(1) {
        assert_eq!(buffer[(detail.right() - 1, y)].symbol(), "│");
    }
}

#[tokio::test]
async fn detail_scroll_reaches_local_checks_after_wrapped_header() {
    let branch = "feature/replace-the-detail-header-with-a-fully-wrapped-branch-name";
    let title = "Make the selected pull request detail header preserve every readable word";
    let (mut app, _) = TestScenario::builder()
        .stack("main", &[branch])
        .pr(branch, PrBuilder::new(1833).title(title))
        .current_branch(branch)
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let area = Rect::new(0, 0, 54, 23);
    let detail = ui::regions(area, &app)
        .detail
        .expect("selected branch has details");
    app.set_detail_viewport(
        usize::from(detail.height.saturating_sub(2)),
        usize::from(detail.width.saturating_sub(2)),
    );
    for _ in 0..20 {
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }

    let buffer = draw(&app, area.width, area.height);
    assert!(find_row(&buffer, "Local checks:").is_some());
    assert!(app.detail_scroll() > 0);
}

#[tokio::test]
async fn no_pr_details_do_not_scroll_past_visible_local_evidence() {
    let (mut app, _) = TestScenario::builder().single("main", "feature").into_app();
    app.poll_once(Source::All).await;

    let area = Rect::new(0, 0, 120, 23);
    let detail = ui::regions(area, &app)
        .detail
        .expect("no-PR branch has details");
    app.set_detail_viewport(
        usize::from(detail.height.saturating_sub(2)),
        usize::from(detail.width.saturating_sub(2)),
    );
    let buffer = draw(&app, area.width, area.height);
    assert!(find_row(&buffer, "no PR yet").is_some());
    assert!(find_row(&buffer, "Local checks:").is_some());
    assert!(find_row(&buffer, "none declared or reported").is_some());

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll(), 0, "all no-PR details already fit");
}
