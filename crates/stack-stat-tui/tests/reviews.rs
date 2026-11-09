mod support;

use ratatui::{Terminal, backend::TestBackend};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{ConfigBuilder, PrBuilder, TestScenario, assert_row_contains, find_body_row};

#[tokio::test]
async fn review_column_is_yellow_text_only_when_a_review_is_required() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["none", "required", "requested"])
        .pr("none", PrBuilder::new(1).ready())
        .pr("required", PrBuilder::new(2).ready().review_required())
        .pr(
            "requested",
            PrBuilder::new(3).ready().review_required().pending_review(),
        )
        .current_branch("none")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let buffer = terminal.backend().buffer();

    let none = find_body_row(buffer, "#1").unwrap();
    let required = find_body_row(buffer, "#2").unwrap();
    let requested = find_body_row(buffer, "#3").unwrap();
    assert_row_contains(buffer, none, &["#1", "—"]);
    assert_row_contains(buffer, required, &["#2", "◷ required"]);
    assert_row_contains(buffer, requested, &["#3", "◷ pending"]);

    let colored = |row: u16, text: &str| {
        let line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect();
        let column = line.find(text).expect("text on row");
        let x = line[..column].chars().count() as u16;
        buffer[(x, row)].fg
    };
    assert_eq!(colored(required, "◷ required"), theme.warning);
    assert_eq!(colored(requested, "◷ pending"), theme.warning);
    assert_ne!(colored(none, "—"), theme.warning);
}

#[tokio::test]
async fn github_approval_by_a_hidden_bot_is_not_masked_by_pending_requests() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["botok"])
        .pr(
            "botok",
            PrBuilder::new(1)
                .ready()
                .approved()
                .bot_review("copilot-pull-request-reviewer")
                .pending_review(),
        )
        .config(ConfigBuilder::new().show_bots(false))
        .current_branch("botok")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let buffer = terminal.backend().buffer();

    let row = find_body_row(buffer, "#1").unwrap();
    assert_row_contains(buffer, row, &["#1", "✔ approved (bot)"]);
}
