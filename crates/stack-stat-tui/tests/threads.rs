mod support;

use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui::{
        self,
        animation::{CellId, Column, diff_cells},
    },
};
use support::{
    ConfigBuilder, FakeApp, PrBuilder, TestScenario, assert_row_contains, find_body_row, find_row,
};

fn draw(app: &FakeApp, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::TrueColor)))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn color_for_text(buffer: &Buffer, row: u16, text: &str) -> Color {
    let line: String = (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect();
    let column = line.find(text).expect("text on row");
    let x = line[..column].chars().count() as u16;
    buffer[(x, row)].fg
}

async fn app_with_threads(gate: bool, unresolved_threads: usize) -> FakeApp {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr(
            "auth",
            PrBuilder::new(42)
                .checks_all_pass()
                .approved()
                .unresolved_threads(unresolved_threads),
        )
        .current_branch("auth")
        .config(ConfigBuilder::new().gate_on_unresolved_threads(gate))
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    app
}

#[tokio::test]
async fn threads_column_shows_success_for_zero_unresolved_threads() {
    let app = app_with_threads(false, 0).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 120, 40);
    let row = find_body_row(&buffer, "#42").expect("branch row");

    assert_row_contains(&buffer, row, &["✔ 0"]);
    let detail_row = find_row(&buffer, "Unresolved threads: 0").expect("thread detail");
    assert_eq!(
        color_for_text(&buffer, detail_row, "Unresolved threads: 0"),
        theme.success
    );
    assert!(find_row(&buffer, "Reviews:").expect("reviews section") < detail_row);
    assert!(detail_row < find_row(&buffer, "Local checks:").expect("local section"));
}

#[tokio::test]
async fn gated_unresolved_threads_block_the_branch_and_are_red() {
    let app = app_with_threads(true, 3).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 120, 40);
    let row = find_body_row(&buffer, "#42").expect("branch row");
    let line: String = (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect();
    let verdict = line.split("auth").next().expect("branch prefix");

    assert_row_contains(&buffer, row, &["✖ 3"]);
    assert_eq!(color_for_text(&buffer, row, "✖ 3"), theme.error);
    assert!(verdict.contains('✖'));
    let detail_row = find_row(&buffer, "Unresolved threads: 3").expect("thread detail");
    assert_eq!(
        color_for_text(&buffer, detail_row, "Unresolved threads: 3"),
        theme.error
    );
}

#[tokio::test]
async fn non_gating_unresolved_threads_warn_without_blocking_the_branch() {
    let app = app_with_threads(false, 3).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 120, 40);
    let row = find_body_row(&buffer, "#42").expect("branch row");
    let line: String = (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect();
    let verdict = line.split("auth").next().expect("branch prefix");

    assert_row_contains(&buffer, row, &["… 3"]);
    assert_eq!(color_for_text(&buffer, row, "… 3"), theme.warning);
    assert!(verdict.contains('✔'));
    let detail_row = find_row(&buffer, "Unresolved threads: 3").expect("thread detail");
    assert_eq!(
        color_for_text(&buffer, detail_row, "Unresolved threads: 3"),
        theme.warning
    );
}

#[tokio::test]
async fn narrow_threads_header_uses_the_abbreviated_label() {
    let app = app_with_threads(false, 0).await;
    let buffer = draw(&app, 90, 28);

    assert!(find_row(&buffer, "Thr").is_some());
}

#[tokio::test]
async fn changed_unresolved_threads_flash_the_threads_cell() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", PrBuilder::new(42).unresolved_threads(1))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let old = app.state().clone();

    handles
        .github
        .set_prs(vec![PrBuilder::new(42).unresolved_threads(2).build()]);
    app.poll_once(Source::GitHub).await;

    assert_eq!(
        diff_cells(&old, app.state()),
        vec![CellId::new("auth", Column::Threads)]
    );
    assert!(
        app.animation()
            .is_flashing(&CellId::new("auth", Column::Threads))
    );
}
