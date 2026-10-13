mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Color};
use stack_stat_core::{
    compose::StoreSnapshot,
    providers::{GitHubError, ProviderAction},
};
use stack_stat_tui::{
    app::Mode,
    message::{ActionSteps, AppMessage, Source},
    theme::{ColorDepth, Theme},
    ui,
};
use support::{FakeApp, PrBuilder, TestScenario, assert_row_contains, find_row};

fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}

fn draw(app: &FakeApp, width: u16, height: u16, theme: &Theme) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, theme))
        .unwrap();
    terminal.backend().buffer().clone()
}

async fn ready(with_pr: bool) -> FakeApp {
    let mut scenario = TestScenario::builder()
        .stack("main", &["feature/auth"])
        .current_branch("feature/auth");
    if with_pr {
        scenario = scenario.pr("feature/auth", PrBuilder::new(42));
    }
    let (mut app, _) = scenario.build().into_app();
    app.poll_once(Source::All).await;
    app
}

fn mutation(app: &mut FakeApp, result: Result<String, GitHubError>) {
    app.apply(AppMessage::MutationDone {
        steps: ActionSteps::single(ProviderAction::SyncStack),
        result,
    });
}

fn assert_text_color(buffer: &Buffer, row: u16, text: &str, color: Color) {
    let symbols: Vec<_> = text.chars().map(|ch| ch.to_string()).collect();
    let start = (0..buffer.area.width)
        .find(|&x| {
            symbols.iter().enumerate().all(|(offset, symbol)| {
                let column = usize::from(x) + offset;
                column < usize::from(buffer.area.width)
                    && buffer[(column as u16, row)].symbol() == symbol.as_str()
            })
        })
        .unwrap_or_else(|| panic!("missing {text:?} on row {row}"));
    for offset in 0..symbols.len() {
        assert_eq!(buffer[(start + offset as u16, row)].fg, color);
    }
}

#[tokio::test]
async fn confirmation_keeps_existing_notice_on_a_separate_footer_line() {
    let mut app = ready(true).await;
    mutation(&mut app, Ok("Saved successfully".into()));
    app.handle_key(key('p'));
    let Mode::Confirm(prompt) = app.mode() else {
        panic!("push must request confirmation");
    };
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 120, 30, &theme);
    let prompt_row = find_row(&buffer, &prompt.title).expect("prompt title");
    let notice_row = find_row(&buffer, "Saved successfully").expect("retained notice");
    assert_row_contains(&buffer, prompt_row, &[&prompt.title, "y/n"]);
    assert_ne!(prompt_row, notice_row);
    assert_eq!(notice_row, buffer.area.height - 1);
    assert_text_color(&buffer, notice_row, "Saved successfully", theme.muted);
    let footer = ui::regions(buffer.area, &app).status;
    assert!(prompt_row >= footer.y);
    assert!(footer.height >= 2);
}

#[tokio::test]
async fn confirmation_without_notice_does_not_copy_prompt_into_feedback() {
    let mut app = ready(true).await;
    app.handle_key(key('p'));
    assert!(app.notice().is_none());
    let Mode::Confirm(prompt) = app.mode() else {
        panic!("push must request confirmation");
    };
    let buffer = draw(&app, 120, 30, &Theme::for_depth(ColorDepth::Ansi256));
    let prompt_row = find_row(&buffer, &prompt.title).expect("prompt title");
    assert!(prompt_row < buffer.area.height - 1);
    assert_row_contains(&buffer, buffer.area.height - 1, &["y confirm", "n cancel"]);
}

#[tokio::test]
async fn notice_levels_use_semantic_colors_at_every_supported_depth() {
    for depth in [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Basic16,
    ] {
        let theme = Theme::for_depth(depth);
        let mut app = ready(true).await;
        mutation(&mut app, Ok("Saved successfully".into()));
        let buffer = draw(&app, 120, 30, &theme);
        assert_text_color(&buffer, 29, "Saved successfully", theme.muted);

        app.apply(AppMessage::Store(Ok(StoreSnapshot {
            entries: vec![],
            warnings: vec!["Unreadable report".into()],
            fetched_at: app.now(),
        })));
        let buffer = draw(&app, 120, 30, &theme);
        assert_text_color(&buffer, 29, "Unreadable report", theme.warning);

        mutation(
            &mut app,
            Err(GitHubError::Network("Network unavailable".into())),
        );
        let buffer = draw(&app, 120, 30, &theme);
        assert_text_color(&buffer, 29, "Network unavailable", theme.error);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.handle_key(key('p'));
        app.handle_key(key('y'));
        let notice = app.notice().expect("running action reports progress");
        let buffer = draw(&app, 120, 30, &theme);
        assert_text_color(&buffer, 29, &notice.text, theme.accent);
        assert_text_color(
            &buffer,
            29,
            &app.animation().spinner_char().to_string(),
            theme.accent,
        );
    }
}

#[tokio::test]
async fn link_prompt_displays_tracking_detail_and_exact_ordered_commands() {
    for (action, command) in [
        (ProviderAction::PushStack, "gh stack push"),
        (ProviderAction::SyncStack, "gh stack sync"),
    ] {
        let mut app = ready(true).await;
        app.apply(AppMessage::MutationDone {
            steps: ActionSteps::single(action),
            result: Err(GitHubError::LocalTrackingMissing),
        });
        let Mode::Confirm(prompt) = app.mode() else {
            panic!("missing tracking must request link confirmation");
        };
        let buffer = draw(&app, 120, 30, &Theme::for_depth(ColorDepth::TrueColor));
        let title = find_row(&buffer, &prompt.title).expect("link title");
        assert_row_contains(&buffer, title, &[&prompt.title, "y/n"]);
        assert!(
            find_row(&buffer, &prompt.detail).is_some(),
            "link detail is visible"
        );
        let checkout = find_row(&buffer, "gh stack checkout 42").expect("exact PR checkout");
        let action = find_row(&buffer, command).expect("final stack command");
        assert!(title < checkout && checkout < action);
        assert!(
            action < buffer.area.height - 1,
            "notice/hints remain separate"
        );
        let footer = ui::regions(buffer.area, &app).status;
        assert!(footer.height >= 5);
    }
}

#[tokio::test]
async fn link_prompt_without_pr_displays_exact_branch_checkout_target() {
    let mut app = ready(false).await;
    mutation(&mut app, Err(GitHubError::LocalTrackingMissing));
    let buffer = draw(&app, 120, 30, &Theme::for_depth(ColorDepth::TrueColor));
    let checkout = find_row(&buffer, "gh stack checkout feature/auth").expect("branch checkout");
    let sync = find_row(&buffer, "gh stack sync").expect("sync command");
    assert!(checkout < sync);
}

#[tokio::test]
async fn missing_extension_error_is_visible_in_the_footer_with_error_color() {
    let mut app = ready(true).await;
    mutation(&mut app, Err(GitHubError::MissingStackExtension));
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 200, 30, &theme);
    let hint = "gh extension install github/gh-stack";
    assert_row_contains(&buffer, 29, &["gh-stack extension missing", hint]);
    assert_text_color(&buffer, 29, hint, theme.error);
    assert!(
        !matches!(app.mode(), Mode::Confirm(_)),
        "no link prompt for a missing extension"
    );
}

#[tokio::test]
async fn idle_update_fallback_is_muted_and_yields_to_a_notice() {
    let mut app = ready(true).await;
    app.apply(AppMessage::UpdateAvailable("9.8.7".into()));
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 120, 30, &theme);
    assert_text_color(&buffer, 29, "stack-stat 9.8.7 available", theme.muted);
    assert_row_contains(
        &buffer,
        29,
        &["quit", "stack-stat 9.8.7", "stack-stat update"],
    );

    mutation(&mut app, Ok("Saved successfully".into()));
    let buffer = draw(&app, 120, 30, &theme);
    assert!(find_row(&buffer, "9.8.7").is_none());
    assert_text_color(&buffer, 29, "Saved successfully", theme.muted);
}

#[tokio::test]
async fn narrow_notice_truncates_without_overwriting_contextual_hints() {
    let mut app = ready(true).await;
    mutation(
        &mut app,
        Err(GitHubError::Network("échec réseau très long ".repeat(8))),
    );
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, 32, 20, &theme);
    assert_row_contains(&buffer, 19, &["quit", "échec", "…"]);
    assert_eq!(buffer[(31, 19)].fg, theme.error);
}

#[tokio::test]
async fn tiny_terminals_keep_regions_bounded_and_prompt_feedback_separate_when_possible() {
    let mut app = ready(true).await;
    mutation(&mut app, Err(GitHubError::LocalTrackingMissing));
    let theme = Theme::for_depth(ColorDepth::Basic16);
    for width in [0, 1, 2, 8, 20, 40] {
        for height in [0, 1, 2, 3, 5, 8] {
            let area = Rect::new(0, 0, width, height);
            let regions = ui::regions(area, &app);
            for region in [regions.header, regions.table, regions.status] {
                assert!(region.right() <= area.right());
                assert!(region.bottom() <= area.bottom());
            }
            assert!(regions.header.bottom() <= regions.table.y);
            assert!(regions.table.bottom() <= regions.status.y);
            let _ = draw(&app, width, height, &theme);
        }
    }
    let buffer = draw(&app, 40, 2, &theme);
    assert_row_contains(&buffer, 0, &["y/n"]);
    assert_row_contains(&buffer, 1, &["y confirm", "n cancel"]);
}

#[tokio::test]
async fn normal_and_filter_modes_preserve_contextual_hints() {
    let mut app = ready(true).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let wide = draw(&app, 120, 30, &theme);
    assert_row_contains(
        &wide,
        29,
        &["previous", "scroll details down", "checkout branch", "quit"],
    );
    let compact = draw(&app, 80, 30, &theme);
    assert_row_contains(&compact, 29, &["next", "open PR", "help", "quit"]);
    assert!(find_row(&compact, "checkout branch").is_none());

    mutation(&mut app, Ok("Saved successfully".into()));
    app.handle_key(key('/'));
    app.handle_key(key('a'));
    let filtered = draw(&app, 120, 30, &theme);
    assert_row_contains(
        &filtered,
        29,
        &["Filter: a", "Enter keep", "Esc clear", "Saved successfully"],
    );
}
