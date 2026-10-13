mod support;

use ratatui::{Terminal, backend::TestBackend, style::Color};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{PrBuilder, TestScenario, find_row};

type Check = (&'static str, Option<&'static str>);
const PASS: Check = ("COMPLETED", Some("SUCCESS"));
const FAIL: Check = ("COMPLETED", Some("FAILURE"));
const CANCELLED: Check = ("COMPLETED", Some("CANCELLED"));
const TIMED_OUT: Check = ("COMPLETED", Some("TIMED_OUT"));
const SKIPPED: Check = ("COMPLETED", Some("SKIPPED"));
const RUNNING: Check = ("IN_PROGRESS", None);
const QUEUED: Check = ("QUEUED", None);

/// Render a one-branch stack whose PR has exactly these checks and return the Checks cell text,
/// the color of its first character, and the spinner glyph in use for that frame.
async fn checks_cell(checks: &[Check]) -> (String, Color, char) {
    let mut pr = PrBuilder::new(1);
    for (index, (status, conclusion)) in checks.iter().enumerate() {
        pr = pr.check(&format!("check-{index}"), status, *conclusion);
    }
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", pr)
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
    let row = find_row(buffer, "#1").unwrap();
    let line: Vec<&str> = (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect();
    // The Checks cell starts after the 7-wide State column and one column of spacing.
    let start = line
        .iter()
        .position(|symbol| *symbol == "O")
        .expect("state column")
        + 8;
    let cell: String = line[start..start + 9].concat();
    (
        cell.trim_end().to_owned(),
        buffer[(start as u16, row)].fg,
        app.animation().spinner_char(),
    )
}

enum Expect {
    /// Literal cell text.
    Text(&'static str, Color),
    /// A settled icon followed by a count.
    Icon(char, &'static str, Color),
    /// The animated spinner (replacing the icon) followed by a count.
    Spinner(&'static str, Color),
}

#[tokio::test]
async fn checks_cell_covers_every_combination_of_check_states() {
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let cases: Vec<(&str, Vec<Check>, Expect)> = vec![
        ("none", vec![], Expect::Text("—", theme.muted)),
        (
            "only skipped",
            vec![SKIPPED, SKIPPED],
            Expect::Text("—", theme.muted),
        ),
        (
            "all passed",
            vec![PASS, PASS],
            Expect::Icon('✔', "2/2", theme.success),
        ),
        (
            "failed",
            vec![FAIL, PASS],
            Expect::Icon('✖', "1/2", theme.error),
        ),
        (
            "cancelled",
            vec![CANCELLED],
            Expect::Icon('✖', "0/1", theme.error),
        ),
        (
            "timed out",
            vec![TIMED_OUT, PASS],
            Expect::Icon('✖', "1/2", theme.error),
        ),
        (
            "running",
            vec![RUNNING],
            Expect::Spinner("0/1", theme.warning),
        ),
        (
            "queued",
            vec![QUEUED],
            Expect::Spinner("0/1", theme.warning),
        ),
        (
            "passed and running",
            vec![PASS, RUNNING],
            Expect::Spinner("1/2", theme.warning),
        ),
        (
            "skipped and running",
            vec![SKIPPED, RUNNING],
            Expect::Spinner("0/1", theme.warning),
        ),
        (
            "failed and running",
            vec![FAIL, RUNNING],
            Expect::Spinner("0/2", theme.error),
        ),
        (
            "failed, queued and passed",
            vec![FAIL, QUEUED, PASS],
            Expect::Spinner("1/3", theme.error),
        ),
    ];
    for (name, checks, expect) in cases {
        let (cell, color, spinner) = checks_cell(&checks).await;
        let (expected_text, expected_color) = match expect {
            Expect::Text(text, color) => (text.to_owned(), color),
            Expect::Icon(icon, count, color) => (format!("{icon} {count}"), color),
            Expect::Spinner(count, color) => (format!("{spinner} {count}"), color),
        };
        assert_eq!(cell, expected_text, "{name}");
        assert_eq!(color, expected_color, "{name}");
    }
}
