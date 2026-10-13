mod support;

use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use stack_stat_tui::{
    app::PointerTarget,
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{PrBuilder, TestScenario, assert_row_contains, find_body_row, find_row};

fn draw(app: &support::FakeApp, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::Ansi256)))
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn header_expands_only_in_wide_tall_terminals() {
    let (app, _) = TestScenario::builder().build().into_app();

    assert_eq!(ui::regions(Rect::new(0, 0, 70, 30), &app).header.height, 8);
    assert_eq!(ui::regions(Rect::new(0, 0, 90, 44), &app).header.height, 10);
    assert_eq!(ui::regions(Rect::new(0, 0, 69, 30), &app).header.height, 5);
    assert_eq!(ui::regions(Rect::new(0, 0, 70, 29), &app).header.height, 5);
}

#[tokio::test]
async fn selected_branch_always_has_a_detail_pane_that_fills_the_remaining_body() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let area = Rect::new(0, 0, 100, 28);
    let regions = ui::regions(area, &app);
    let detail = regions.detail.expect("selected branch needs details");
    assert_eq!(detail.y, regions.table.bottom());
    assert_eq!(detail.bottom(), regions.status.y);
    assert!(find_row(&draw(&app, 100, 28), "Reviews:").is_some());
}

#[tokio::test]
async fn table_displays_go_wait_and_no_go_verdict_glyphs() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["go", "wait", "no-go"])
        .pr(
            "go",
            PrBuilder::new(42).ready().checks_all_pass().approved(),
        )
        .pr("wait", PrBuilder::new(43).check("ci", "IN_PROGRESS", None))
        .pr("no-go", PrBuilder::new(44).draft())
        .current_branch("go")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let buffer = draw(&app, 120, 40);
    for (number, glyph) in [(42, "✔"), (43, "◷"), (44, "✖")] {
        let row = find_body_row(&buffer, &format!("#{number}")).expect("branch row");
        assert_row_contains(&buffer, row, &[glyph, &format!("#{number}")]);
    }
}

#[tokio::test]
async fn local_column_appears_only_when_local_checks_exist() {
    let header_has_local = |app: &support::FakeApp| {
        let buffer = draw(app, 120, 40);
        let row = find_row(&buffer, "Reviews").expect("table header row");
        let line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect();
        line.contains("Local")
    };
    let (mut without, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", PrBuilder::new(42))
        .current_branch("auth")
        .build()
        .into_app();
    without.poll_once(Source::All).await;
    assert!(!header_has_local(&without));

    let (mut with, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", PrBuilder::new(42))
        .current_branch("auth")
        .config(support::ConfigBuilder::new().local("lint"))
        .build()
        .into_app();
    with.poll_once(Source::All).await;
    assert!(header_has_local(&with));
}

#[tokio::test]
async fn clicking_a_branch_title_line_selects_its_branch() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let area = Rect::new(0, 0, 100, 28);
    let regions = ui::regions(area, &app);
    let title_line = regions.table.y + 5;
    assert_eq!(
        ui::hit_test(area, &app, regions.table.x + 4, title_line),
        PointerTarget::TableRow(0)
    );
}
