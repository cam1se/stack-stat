mod support;

use chrono::{Duration, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{FakeApp, PrBuilder, TestScenario, find_row};

fn row_backgrounds(app: &FakeApp, theme: &Theme) -> (Color, Color, String) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, theme))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let auth = find_row(buffer, "#42").unwrap();
    let api = find_row(buffer, "#43").unwrap();
    (
        buffer[(2, auth)].bg,
        buffer[(2, api)].bg,
        buffer[(1, auth)].symbol().to_owned(),
    )
}

#[tokio::test]
async fn pulse_follows_selection_and_asterisk_follows_the_worktree() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("api"));

    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let start = Utc.timestamp_opt(1_760_000_000, 0).single().unwrap();
    app.tick(start);
    let (auth_before, api_before, marker) = row_backgrounds(&app, &theme);
    app.tick(start + Duration::milliseconds(1_000));
    let (auth_after, api_after, _) = row_backgrounds(&app, &theme);

    assert_eq!(marker, "*", "the worktree branch carries the asterisk");
    assert_eq!(
        auth_before, auth_after,
        "an unselected worktree row must not animate"
    );
    assert_ne!(api_before, api_after, "the selected row must animate");
}
