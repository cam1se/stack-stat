mod support;

use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{ConfigBuilder, FakeApp, PrBuilder, TestScenario, find_body_row, find_row};

fn draw(app: &FakeApp, width: u16) -> (Buffer, Theme) {
    let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    terminal
        .draw(|frame| ui::render(frame, app, &theme))
        .unwrap();
    (terminal.backend().buffer().clone(), theme)
}

fn row_text(buffer: &Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect()
}

fn text_color(buffer: &Buffer, row: u16, text: &str) -> Color {
    let line = row_text(buffer, row);
    let start = line.find(text).expect("text should be rendered");
    let start = line[..start].chars().count() as u16;
    buffer[(start, row)].fg
}

async fn app_with_rebase(branch: &str, gate_trunk_rebase: bool, current: &str) -> FakeApp {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["base", "feature"])
        .remote_rebase(branch)
        .pr("base", PrBuilder::new(1).checks_all_pass().approved())
        .pr("feature", PrBuilder::new(2).checks_all_pass().approved())
        .current_branch(current)
        .config(ConfigBuilder::new().rebase_onto_trunk(gate_trunk_rebase))
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    app
}

#[tokio::test]
async fn rebase_column_is_hidden_when_no_layer_needs_rebase() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["base", "feature"])
        .current_branch("feature")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let (buffer, _) = draw(&app, 120);
    assert!(find_row(&buffer, "Rebase").is_none());
}

#[tokio::test]
async fn parent_rebase_is_blocking_and_names_the_layer_below() {
    let app = app_with_rebase("feature", false, "feature").await;
    let (buffer, theme) = draw(&app, 120);
    let row = find_body_row(&buffer, "#2").expect("feature row");
    assert!(row_text(&buffer, row).contains('↻'));
    assert!(!row_text(&buffer, row).contains("below"));
    assert_eq!(text_color(&buffer, row, "↻"), theme.error);
    assert!(row_text(&buffer, row).contains('✖'));
    let detail = find_row(&buffer, "Rebase needed: onto base").expect("parent rebase detail");
    assert_eq!(
        text_color(&buffer, detail, "Rebase needed: onto base"),
        theme.error
    );
}

#[tokio::test]
async fn rebase_trigger_and_every_layer_above_it_are_blocked() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["a", "b", "c"])
        .remote_rebase("b")
        .pr("a", PrBuilder::new(1).checks_all_pass().approved())
        .pr("b", PrBuilder::new(2).checks_all_pass().approved())
        .pr("c", PrBuilder::new(3).checks_all_pass().approved())
        .current_branch("a")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let (buffer, theme) = draw(&app, 120);

    let below = find_body_row(&buffer, "#1").expect("a row");
    assert!(!row_text(&buffer, below).contains('↻'));
    for pr in ["#2", "#3"] {
        let row = find_body_row(&buffer, pr).expect("row");
        assert_eq!(text_color(&buffer, row, "↻"), theme.error, "{pr}");
        assert!(row_text(&buffer, row).contains('✖'), "{pr}");
    }
}

#[tokio::test]
async fn trunk_rebase_is_nonblocking_by_default() {
    let app = app_with_rebase("base", false, "base").await;
    let (buffer, theme) = draw(&app, 120);
    let row = find_body_row(&buffer, "#1").expect("base row");
    assert!(row_text(&buffer, row).contains("↻ main"));
    assert_eq!(text_color(&buffer, row, "↻ main"), theme.muted);
    assert!(row_text(&buffer, row).contains('✔'));
    let detail = find_row(&buffer, "Behind main: rebase onto trunk (not blocking)")
        .expect("trunk rebase detail");
    assert_eq!(
        text_color(&buffer, detail, "Behind main: rebase onto trunk"),
        theme.muted
    );
}

#[tokio::test]
async fn gated_trunk_rebase_is_blocking() {
    let app = app_with_rebase("base", true, "base").await;
    let (buffer, theme) = draw(&app, 120);
    let row = find_body_row(&buffer, "#1").expect("base row");
    assert_eq!(text_color(&buffer, row, "↻ main"), theme.error);
    assert!(row_text(&buffer, row).contains('✖'));
    let detail = find_row(&buffer, "Behind main: rebase onto trunk").expect("trunk rebase detail");
    assert_eq!(
        text_color(&buffer, detail, "Behind main: rebase onto trunk"),
        theme.error
    );
    assert!(find_row(&buffer, "(not blocking)").is_none());
}

#[tokio::test]
async fn narrow_rebase_column_uses_short_header_and_glyph() {
    let app = app_with_rebase("feature", false, "feature").await;
    let (buffer, _) = draw(&app, 90);
    let header = find_row(&buffer, "Rb").expect("narrow rebase header");
    assert!(!row_text(&buffer, header).contains("Rebase"));
    let row = find_body_row(&buffer, "#2").expect("feature row");
    assert!(row_text(&buffer, row).contains('↻'));
    assert!(!row_text(&buffer, row).contains("below"));
}
