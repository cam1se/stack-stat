mod support;

use ratatui::{Terminal, backend::TestBackend, style::Color};
use stack_stat_core::providers::{StackBranch, StackPr, StackViewResponse};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{FakeApp, PrBuilder, TestScenario, find_row};

fn draw(app: &FakeApp, width: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 28)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::TrueColor)))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn merged_stack() -> StackViewResponse {
    StackViewResponse {
        trunk: "main".into(),
        current_branch: "go".into(),
        branches: [
            ("go", 1, false),
            ("wait", 2, false),
            ("blocked", 3, false),
            ("merged", 4, true),
        ]
        .into_iter()
        .map(|(name, number, is_merged)| StackBranch {
            name: name.into(),
            head: Some(format!("{number:040x}")),
            base: "0".repeat(40),
            is_current: name == "go",
            is_merged,
            is_queued: false,
            needs_rebase: false,
            pr: Some(StackPr {
                number,
                url: format!("https://example.test/pr/{number}"),
                state: if is_merged { "MERGED" } else { "OPEN" }.into(),
            }),
        })
        .collect(),
    }
}

async fn dashboard() -> FakeApp {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["go", "wait", "blocked", "merged"])
        .pr("go", PrBuilder::new(1).ready().checks_all_pass().approved())
        .pr("wait", PrBuilder::new(2).check("ci", "IN_PROGRESS", None))
        .pr("blocked", PrBuilder::new(3).draft())
        .pr(
            "merged",
            PrBuilder::new(4).ready().checks_all_pass().approved(),
        )
        .current_branch("go")
        .build()
        .into_app();
    handles.github.set_stack(merged_stack());
    app.poll_once(Source::All).await;
    app
}

#[tokio::test]
async fn summary_strip_counts_each_verdict() {
    let app = dashboard().await;
    let buffer = draw(&app, 120);
    let summary = find_row(&buffer, "✔ 1   … 1   ✖ 1   ✓ 1").expect("summary strip is rendered");
    assert!(summary > 0);
}

#[tokio::test]
async fn summary_strip_hides_zero_merged_group() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["go"])
        .pr("go", PrBuilder::new(1).ready().checks_all_pass().approved())
        .current_branch("go")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    assert!(find_row(&draw(&app, 120), "merged").is_none());
}

#[tokio::test]
async fn footer_keeps_quit_hint_at_compact_widths() {
    let app = dashboard().await;
    for width in [80, 60] {
        assert!(
            find_row(&draw(&app, width), "quit").is_some(),
            "quit hint missing at {width} columns"
        );
    }
}

#[test]
fn basic_ansi_theme_uses_no_rgb_colors() {
    let theme = Theme::for_depth(ColorDepth::Basic16);
    for color in [
        theme.success,
        theme.error,
        theme.warning,
        theme.muted,
        theme.active_base,
        theme.active_peak,
        theme.selected,
        theme.flash,
        theme.skeleton,
        theme.header_title,
    ] {
        assert!(
            !matches!(color, Color::Rgb(_, _, _)),
            "unexpected RGB color"
        );
    }
}
