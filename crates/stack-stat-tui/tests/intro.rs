mod support;

use std::time::Duration;

use ratatui::{Terminal, backend::TestBackend, style::Color};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui::{self, intro::Intro},
};
use support::{FakeApp, PrBuilder, TestScenario, find_row};

const LARGE_WIDTH: u16 = 120;
const LARGE_HEIGHT: u16 = 40;

fn draw_with_intro(app: &FakeApp, intro: &mut Intro, theme: Theme) -> ratatui::buffer::Buffer {
    draw_with_intro_size(app, intro, theme, LARGE_WIDTH, LARGE_HEIGHT)
}

fn draw_with_intro_size(
    app: &FakeApp,
    intro: &mut Intro,
    theme: Theme,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            ui::render(frame, app, &theme);
            intro.apply(
                frame.buffer_mut(),
                area,
                ui::regions(area, app),
                app.visible_indices().len(),
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn draw_dashboard(app: &FakeApp, theme: Theme) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(LARGE_WIDTH, LARGE_HEIGHT)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &theme))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn assert_no_rgb_colors(buffer: &ratatui::buffer::Buffer) {
    for cell in buffer.content() {
        assert!(
            !matches!(cell.fg, Color::Rgb(_, _, _)) && !matches!(cell.bg, Color::Rgb(_, _, _)),
            "intro frame contains an RGB color"
        );
    }
}

#[test]
fn splash_covers_the_dashboard_until_github_reports() {
    let (app, _) = TestScenario::builder()
        .stack("main", &["base", "tip"])
        .current_branch("tip")
        .build()
        .into_app();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let mut intro = Intro::new(theme);

    for _ in 0..10 {
        intro.advance(Duration::from_secs(1), false);
        let buffer = draw_with_intro(&app, &mut intro, theme);
        assert!(
            find_row(&buffer, "Stack").is_none(),
            "dashboard remains covered"
        );
        assert!(find_row(&buffer, "base").is_none(), "table remains covered");
    }
    assert!(intro.is_active());
}

#[tokio::test]
async fn completed_reveal_leaves_the_plain_dashboard_unchanged() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["base", "middle", "tip"])
        .pr("base", PrBuilder::new(1).checks_all_pass().approved())
        .pr("middle", PrBuilder::new(2).checks_all_pass().approved())
        .pr("tip", PrBuilder::new(3).checks_all_pass().approved())
        .current_branch("tip")
        .build()
        .into_app();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let mut intro = Intro::new(theme);

    app.poll_once(Source::All).await;
    intro.advance(
        Duration::from_millis(1_200),
        app.has_initial_github_result(),
    );

    assert_eq!(
        draw_with_intro(&app, &mut intro, theme),
        draw_dashboard(&app, theme)
    );
}

#[tokio::test]
async fn skipping_intro_shows_the_plain_dashboard_on_the_next_frame() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["base"])
        .pr("base", PrBuilder::new(1).checks_all_pass().approved())
        .current_branch("base")
        .build()
        .into_app();
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let mut intro = Intro::new(theme);

    app.poll_once(Source::All).await;
    intro.skip();

    assert_eq!(
        draw_with_intro(&app, &mut intro, theme),
        draw_dashboard(&app, theme)
    );
}

#[tokio::test]
async fn basic_ansi_intro_frames_use_no_rgb_colors() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["base", "tip"])
        .pr("base", PrBuilder::new(1).checks_all_pass().approved())
        .pr("tip", PrBuilder::new(2).checks_all_pass().approved())
        .current_branch("tip")
        .build()
        .into_app();
    let theme = Theme::for_depth(ColorDepth::Basic16);
    let mut splash = Intro::new(theme);

    splash.advance(Duration::from_millis(1_600), false);
    assert_no_rgb_colors(&draw_with_intro(&app, &mut splash, theme));

    app.poll_once(Source::All).await;
    let mut reveal = Intro::new(theme);
    reveal.advance(Duration::from_millis(440), app.has_initial_github_result());
    assert_no_rgb_colors(&draw_with_intro(&app, &mut reveal, theme));
}

#[test]
fn compact_wordmark_fallback_renders_on_small_terminals() {
    let (app, _) = TestScenario::builder()
        .stack("main", &["base"])
        .current_branch("base")
        .build()
        .into_app();
    let theme = Theme::for_depth(ColorDepth::Basic16);
    let mut intro = Intro::new(theme);

    intro.advance(Duration::from_millis(1_600), false);
    let buffer = draw_with_intro_size(&app, &mut intro, theme, 80, 24);

    assert!(find_row(&buffer, "stack-stat").is_some());
    assert!(find_row(&buffer, "Stack").is_none());
}
