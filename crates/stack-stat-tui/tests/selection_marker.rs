mod support;

use chrono::{Duration, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Color};
use stack_stat_core::model::LocalSync;

use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{FakeApp, PrBuilder, TestScenario, find_body_row};

fn row_backgrounds(app: &FakeApp, theme: &Theme) -> (Color, Color, String) {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, theme))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let auth = find_body_row(buffer, "#42").unwrap();
    let api = find_body_row(buffer, "#43").unwrap();
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

#[tokio::test]
async fn local_sync_gutter_labels_only_the_current_worktree_branch() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    app.poll_once(Source::All).await;

    for (sync, marker) in [
        (LocalSync::InSync, "*"),
        (LocalSync::Ahead, "*↑"),
        (LocalSync::Behind, "*↓"),
        (LocalSync::Diverged, "*⇅"),
        (LocalSync::Unpushed, "*?"),
    ] {
        handles.git.set_current_sync(sync);
        app.poll_once(Source::Git).await;
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| ui::render(frame, &app, &theme))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let auth = find_body_row(buffer, "#42").unwrap();
        let auth_line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, auth)].symbol())
            .collect();
        let auth_gutter = &auth_line[..auth_line.find("Change 42").unwrap()];
        assert!(
            auth_gutter.contains(marker),
            "expected {marker:?} in current branch gutter: {auth_line}"
        );
        if marker != "*" {
            let marker_offset =
                u16::try_from(auth_line[..auth_line.find(marker).unwrap()].chars().count())
                    .unwrap();
            assert_eq!(buffer[(marker_offset + 1, auth)].fg, theme.muted);
        }

        let api = find_body_row(buffer, "#43").unwrap();
        let api_line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, api)].symbol())
            .collect();
        let api_gutter = &api_line[..api_line.find("Change 43").unwrap()];
        assert!(
            !api_gutter.contains('*'),
            "non-current branch must have an empty gutter: {api_line}"
        );
    }
}
