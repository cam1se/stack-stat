mod support;

use ratatui::{Terminal, backend::TestBackend};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{PrBuilder, TestScenario, find_row};

#[tokio::test]
async fn running_checks_keep_a_warning_colored_spinner_behind_a_failure() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr(
            "auth",
            PrBuilder::new(1)
                .check("build", "COMPLETED", Some("FAILURE"))
                .check("test", "IN_PROGRESS", None),
        )
        .pr(
            "api",
            PrBuilder::new(2).check("build", "COMPLETED", Some("FAILURE")),
        )
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, &app, &theme))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let spinner = app.animation().spinner_char();

    let line = |row: u16| -> Vec<&str> {
        (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect()
    };
    let failing_and_running = line(find_row(buffer, "#1").unwrap());
    let failing_only = line(find_row(buffer, "#2").unwrap());

    let failure_icon = failing_and_running.iter().position(|s| *s == "✖").unwrap();
    // The failure icon stays first; the spinner follows the count in the warning color.
    let spinner_x = failing_and_running[failure_icon..]
        .iter()
        .position(|symbol| symbol.starts_with(spinner))
        .expect("spinner after the failure summary")
        + failure_icon;
    assert_eq!(
        buffer[(spinner_x as u16, find_row(buffer, "#1").unwrap())].fg,
        theme.warning
    );
    assert!(
        !failing_only
            .iter()
            .any(|symbol| symbol.starts_with(spinner)),
        "a row with no running checks must not show a spinner"
    );
}
