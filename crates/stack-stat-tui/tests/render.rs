mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use stack_stat_core::{
    model::StackStatus,
    providers::{GitHubError, ReportedState},
};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{ConfigBuilder, FakeApp, PrBuilder, TestScenario, assert_row_contains, find_row};

fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}
fn draw(app: &FakeApp, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::TrueColor)))
        .unwrap();
    terminal.backend().buffer().clone()
}
async fn ready() -> (FakeApp, support::Handles) {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api", "ui", "docs"])
        .pr(
            "auth",
            PrBuilder::new(42).ready().checks_all_pass().approved(),
        )
        .pr(
            "api",
            PrBuilder::new(43)
                .draft()
                .check("ci/test", "IN_PROGRESS", None)
                .pending_review(),
        )
        .pr(
            "ui",
            PrBuilder::new(44)
                .ready()
                .check("ci/build", "COMPLETED", Some("FAILURE"))
                .changes_requested(),
        )
        .pr(
            "docs",
            PrBuilder::new(45).ready().checks_all_pass().approved(),
        )
        .current_branch("auth")
        .config(ConfigBuilder::new().local("lint").local("test"))
        .local_check("auth", &format!("{:040x}", 1), "lint", ReportedState::Pass)
        .local_check("auth", &format!("{:040x}", 1), "test", ReportedState::Pass)
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    (app, handles)
}

#[tokio::test]
async fn four_layer_healthy_stack_has_passing_status_without_staleness() {
    let mut scenario = TestScenario::builder()
        .stack("main", &["auth", "api", "ui", "docs"])
        .current_branch("auth");
    for (branch, number) in [("auth", 42), ("api", 43), ("ui", 44), ("docs", 45)] {
        scenario = scenario.pr(
            branch,
            PrBuilder::new(number).ready().checks_all_pass().approved(),
        );
    }
    let (mut app, _) = scenario.build().into_app();
    app.poll_once(Source::All).await;
    let buf = draw(&app, 120, 40);
    for number in [42, 43, 44, 45] {
        let row = find_row(&buf, &format!("#{number}")).unwrap();
        assert_row_contains(&buf, row, &["✔", "Open", "✔", "1/1", "approved"]);
    }
    assert!(find_row(&buf, "stale").is_none());
}

#[tokio::test]
async fn branch_table_shows_pr_lifecycle_ci_reviews_and_local_results() {
    let (app, _) = ready().await;
    let buf = draw(&app, 120, 40);
    let auth = find_row(&buf, "#42").unwrap();
    assert_row_contains(
        &buf,
        auth,
        &[
            "►", "✔", "auth", "←wt", "#42", "Open", "1/1", "approved", "lint", "test",
        ],
    );
    assert_row_contains(&buf, auth + 1, &["Change 42"]);
    let api = find_row(&buf, "#43").unwrap();
    let spinner = app.animation().spinner_char().to_string();
    assert_row_contains(
        &buf,
        api,
        &["✖", "api", "#43", "Draft", &spinner, "0/1", "pending"],
    );
    let ui = find_row(&buf, "#44").unwrap();
    assert_row_contains(&buf, ui, &["✖", "ui", "#44", "Open", "✖", "0/1", "changes"]);
    assert!(find_row(&buf, "#45").is_some());
    assert!(find_row(&buf, "stale").is_none());
}

#[tokio::test]
async fn details_are_visible_without_a_toggle_and_reserve_table_height() {
    let (app, _) = ready().await;
    let buf = draw(&app, 120, 40);
    let table_border = (5..buf.area.height)
        .find(|&y| buf[(0, y)].symbol() == "└")
        .unwrap();
    assert!(
        table_border < buf.area.height - 2,
        "details must reserve table height"
    );
    for text in [
        "PR #42",
        "Checks ·",
        "ci/build",
        "Reviews:",
        "Local checks:",
        "lint",
    ] {
        assert!(find_row(&buf, text).is_some(), "missing {text}");
    }
}

#[tokio::test]
async fn skipped_ci_check_is_visible_in_details_not_row_total() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr(
            "auth",
            PrBuilder::new(42)
                .check("ci/build", "COMPLETED", Some("SUCCESS"))
                .check("secrets-scan", "COMPLETED", Some("FAILURE")),
        )
        .current_branch("auth")
        .config(ConfigBuilder::new().skip_check("secrets-scan"))
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let buf = draw(&app, 120, 30);
    assert_row_contains(&buf, find_row(&buf, "#42").unwrap(), &["1/1"]);
    assert!(find_row(&buf, "secrets-scan").is_some());
    assert!(find_row(&buf, "skipped (config)").is_some());
}

#[tokio::test]
async fn bot_review_visibility_toggles_in_detail_pane() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr(
            "auth",
            PrBuilder::new(42)
                .bot_review("dependabot[bot]")
                .pending_review(),
        )
        .current_branch("auth")
        .config(ConfigBuilder::new().bot("dependabot[bot]"))
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    assert!(find_row(&draw(&app, 120, 28), "dependabot[bot]").is_none());
    app.handle_key(key('b'));
    let buf = draw(&app, 120, 28);
    assert!(find_row(&buf, "dependabot[bot]").is_some());
    assert!(find_row(&buf, "(bot)").is_some());
}

#[tokio::test]
async fn branch_switch_and_stale_check_have_distinct_visual_markers() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .config(ConfigBuilder::new().local("lint"))
        .local_check("auth", &"a".repeat(40), "lint", ReportedState::Pass)
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let buf = draw(&app, 120, 40);
    assert!(find_row(&buf, "stale (HEAD moved, was aaaaaaa)").is_some());
    handles.git.set_current_branch("api");
    app.poll_once(Source::Git).await;
    let buf = draw(&app, 120, 40);
    let api = find_row(&buf, "#43").unwrap();
    assert_row_contains(&buf, api, &["api", "←wt"]);
    let auth = find_row(&buf, "#42").unwrap();
    let auth_line: String = (0..buf.area.width)
        .map(|x| buf[(x, auth)].symbol())
        .collect();
    assert!(!auth_line.contains("←wt"));
}

#[tokio::test]
async fn narrow_widths_collapse_review_and_local_columns() {
    let (app, _) = ready().await;
    let wide = draw(&app, 120, 26);
    let medium = draw(&app, 90, 26);
    let narrow = draw(&app, 75, 26);
    assert!(find_row(&wide, "approved").is_some());
    assert!(find_row(&medium, "approved").is_none());
    assert!(find_row(&wide, "lint").is_some());
    assert!(find_row(&narrow, "lint").is_none());
    assert!(find_row(&narrow, "#42").is_some());
}

#[tokio::test]
async fn loading_not_in_stack_help_and_confirmation_render_without_panics() {
    let (mut app, handles) = ready().await;
    // Build a separate App before any GitHub snapshot to exercise placeholders.
    let (mut loading, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .current_branch("auth")
        .build()
        .into_app();
    loading.poll_once(Source::Git).await;
    assert!(find_row(&draw(&loading, 90, 22), "░").is_some());
    app.handle_key(key('?'));
    let help = draw(&app, 120, 32);
    assert!(find_row(&help, "checkout branch").is_some());
    for binding in stack_stat_tui::action::KEYMAP {
        assert!(
            find_row(&help, binding.help).is_some(),
            "missing help for {:?}",
            binding.key
        );
    }
    let compact_help = draw(&app, 80, 24);
    for binding in stack_stat_tui::action::KEYMAP {
        assert!(
            find_row(&compact_help, binding.help).is_some(),
            "short-terminal help hides {:?}",
            binding.key
        );
    }
    app.handle_key(key('?'));
    app.handle_key(key('p'));
    assert!(find_row(&draw(&app, 120, 26), "Push stack? y/n").is_some());
    handles.github.set_stack_error(GitHubError::NotInStack);
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().stack_status, StackStatus::NotInStack);
    assert!(find_row(&draw(&app, 120, 26), "Not in a stack").is_some());
}

#[tokio::test]
async fn overflowing_details_end_with_ellipsis_without_overwriting_footer() {
    let mut pr = PrBuilder::new(42);
    for n in 0..30 {
        pr = pr.check(&format!("ci/check-{n}"), "COMPLETED", Some("SUCCESS"));
    }
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", pr)
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let buf = draw(&app, 80, 24);
    assert!(find_row(&buf, "  …").is_some());
    assert!(find_row(&buf, "quit").is_some());
}

#[tokio::test]
async fn keyboard_scroll_reaches_reviews_after_many_checks_without_changing_selected_branch() {
    let mut pr = PrBuilder::new(42).pending_review();
    for n in 0..40 {
        pr = pr.check(&format!("ci/check-{n}"), "COMPLETED", Some("SUCCESS"));
    }
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", pr)
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .config(ConfigBuilder::new().local("lint"))
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let first = draw(&app, 100, 28);
    assert!(find_row(&first, "ci/check-0").is_some());
    assert!(find_row(&first, "Reviews:").is_none());
    let detail = ui::regions(ratatui::layout::Rect::new(0, 0, 100, 28), &app)
        .detail
        .expect("selected branch has details");
    app.set_detail_viewport_rows(usize::from(detail.height.saturating_sub(2)));
    for _ in 0..8 {
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    }
    let scrolled = draw(&app, 100, 28);
    assert!(find_row(&scrolled, "Reviews:").is_some());
    assert!(find_row(&scrolled, "Local checks:").is_some());
    assert_eq!(app.selected_branch(), Some("auth"));
    assert!(app.detail_scroll() > 0);
    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("auth"));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("api"));
    assert_eq!(app.detail_scroll(), 0);
}

#[tokio::test]
async fn mouse_wheel_scrolls_hovered_pane_and_click_selects_branch() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use stack_stat_tui::app::PointerTarget;
    let mut pr = PrBuilder::new(42);
    for n in 0..30 {
        pr = pr.check(&format!("ci/check-{n}"), "COMPLETED", Some("SUCCESS"));
    }
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", pr)
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let area = ratatui::layout::Rect::new(0, 0, 100, 28);
    let regions = ui::regions(area, &app);
    let detail = regions.detail.unwrap();
    let over_detail = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: detail.x + 2,
        row: detail.y + 2,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        ui::hit_test(area, &app, over_detail.column, over_detail.row),
        PointerTarget::Detail
    );
    app.handle_mouse(over_detail, PointerTarget::Detail);
    assert!(app.detail_scroll() > 0);
    assert_eq!(app.selected_branch(), Some("auth"));
    let over_table = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: regions.table.x + 2,
        row: regions.table.y + 2,
        modifiers: KeyModifiers::NONE,
    };
    app.handle_mouse(over_table, PointerTarget::Table);
    assert_eq!(app.selected_branch(), Some("api"));
    let row = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: regions.table.x + 3,
        row: regions.table.y + 4,
        modifiers: KeyModifiers::NONE,
    };
    let target = ui::hit_test(area, &app, row.column, row.row);
    app.handle_mouse(row, target);
    assert_eq!(app.selected_branch(), Some("auth"));
}

#[test]
fn color_depth_detection_uses_environment_values_without_mutating_them() {
    assert_eq!(
        ColorDepth::detect_from(Some("truecolor"), None),
        ColorDepth::TrueColor
    );
    assert_eq!(
        ColorDepth::detect_from(Some("24bit"), None),
        ColorDepth::TrueColor
    );
    assert_eq!(
        ColorDepth::detect_from(None, Some("xterm-256color")),
        ColorDepth::Ansi256
    );
    assert_eq!(
        ColorDepth::detect_from(None, Some("vt100")),
        ColorDepth::Basic16
    );
}
