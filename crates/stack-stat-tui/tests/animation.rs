mod support;

use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use stack_stat_core::providers::PrResponse;
use stack_stat_tui::{
    message::{AppMessage, Source},
    ui::animation::{AnimationState, CellId, Column, diff_cells, stale_after},
};
use support::{PrBuilder, TestScenario};

#[tokio::test]
async fn completed_check_flashes_only_its_cell_for_five_ticks() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr(
            "auth",
            PrBuilder::new(42).check("ci/test", "IN_PROGRESS", None),
        )
        .current_branch("auth")
        .build()
        .into_app();
    let now = Utc.timestamp_opt(1_760_000_000, 0).single().unwrap();
    app.tick(now);
    app.poll_once(Source::All).await;
    let old = app.state().clone();
    assert!(
        !app.animation()
            .is_flashing(&CellId::new("auth", Column::Checks))
    );
    let done: PrResponse = PrBuilder::new(42)
        .check("ci/test", "COMPLETED", Some("SUCCESS"))
        .build();
    handles.github.set_prs(vec![done]);
    app.poll_once(Source::GitHub).await;
    let changed = diff_cells(&old, app.state());
    assert_eq!(changed, vec![CellId::new("auth", Column::Checks)]);
    let id = CellId::new("auth", Column::Checks);
    assert!(app.animation().is_flashing(&id));
    for n in 1..=5 {
        app.tick(now + ChronoDuration::milliseconds(100 * n));
    }
    assert!(!app.animation().is_flashing(&id));
    app.poll_once(Source::GitHub).await;
    assert!(
        !app.animation().is_flashing(&id),
        "unchanged polls must not re-flash"
    );
}

#[test]
fn spinner_skeleton_and_phase_advance_without_wall_clock() {
    let mut animation = AnimationState::default();
    let spinner = animation.spinner_char();
    let skeleton = animation.skeleton_char(0);
    animation.tick();
    assert_ne!(spinner, animation.spinner_char());
    assert_ne!(skeleton, animation.skeleton_char(0));
    assert!(animation.phase(20) > 0.0);
}

#[test]
fn stale_threshold_is_strictly_greater_than_twice_interval() {
    let last = Utc.timestamp_opt(1_760_000_000, 0).single().unwrap();
    assert!(!stale_after(last + ChronoDuration::seconds(60), last, 30));
    assert!(stale_after(last + ChronoDuration::seconds(61), last, 30));
}

#[tokio::test]
async fn activity_flag_tracks_fetch_start_and_completion() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .current_branch("auth")
        .build()
        .into_app();
    assert!(!app.is_fetching());
    app.apply(AppMessage::FetchStarted(Source::Git));
    assert!(app.is_fetching());
    app.poll_once(Source::Git).await;
    assert!(!app.is_fetching());
}

#[tokio::test]
async fn basic_color_active_row_pulses_bold_without_changing_background() {
    use ratatui::{Terminal, backend::TestBackend, style::Modifier};
    use stack_stat_tui::{
        theme::{ColorDepth, Theme},
        ui,
    };
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", PrBuilder::new(42))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    let theme = Theme::for_depth(ColorDepth::Basic16);
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let row = support::find_row(terminal.backend().buffer(), "#42").unwrap();
    let first = &terminal.backend().buffer()[(5, row)];
    let base_bg = first.bg;
    let base_bold = first.modifier.contains(Modifier::BOLD);
    let now = Utc.timestamp_opt(1_760_000_000, 0).single().unwrap();
    for n in 0..10 {
        app.tick(now + ChronoDuration::milliseconds(n * 100));
    }
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let next = &terminal.backend().buffer()[(5, row)];
    assert_eq!(next.bg, base_bg);
    assert_ne!(next.modifier.contains(Modifier::BOLD), base_bold);
}

#[tokio::test]
async fn rendered_activity_spinner_and_loading_skeleton_advance() {
    use ratatui::{Terminal, backend::TestBackend};
    use stack_stat_tui::{
        theme::{ColorDepth, Theme},
        ui,
    };
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::Git).await;
    app.apply(AppMessage::FetchStarted(Source::GitHub));
    let mut terminal = Terminal::new(TestBackend::new(90, 22)).unwrap();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let first = terminal.backend().buffer().clone();
    let header = support::find_row(&first, "worktree:").unwrap();
    let skeleton = support::find_row(&first, "░").unwrap();
    let line = |buffer: &ratatui::buffer::Buffer, row| {
        (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect::<String>()
    };
    assert!(line(&first, header).contains(app.animation().spinner_char()));
    app.tick(Utc.timestamp_opt(1_760_000_000, 0).single().unwrap());
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let second = terminal.backend().buffer();
    assert_ne!(line(&first, skeleton), line(second, skeleton));
    assert!(line(second, header).contains(app.animation().spinner_char()));
    app.poll_once(Source::GitHub).await;
    assert!(!app.is_fetching());
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let settled = terminal.backend().buffer();
    assert!(line(settled, header).contains('↻'));
}

#[tokio::test]
async fn running_check_spinner_advances_in_table_and_detail() {
    use ratatui::{Terminal, backend::TestBackend};
    use stack_stat_tui::{
        theme::{ColorDepth, Theme},
        ui,
    };
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr(
            "auth",
            PrBuilder::new(42).check("ci/test", "IN_PROGRESS", None),
        )
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let first = terminal.backend().buffer().clone();
    let row = support::find_row(&first, "ci/test").unwrap();
    let before = app.animation().spinner_char();
    assert!((0..first.area.width).any(|x| first[(x, row)].symbol() == before.to_string()));
    app.tick(Utc.timestamp_opt(1_760_000_000, 0).single().unwrap());
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let after = app.animation().spinner_char();
    assert_ne!(before, after);
    let second = terminal.backend().buffer();
    assert!((0..second.area.width).any(|x| second[(x, row)].symbol() == after.to_string()));
}
