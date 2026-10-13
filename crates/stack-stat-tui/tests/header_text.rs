mod support;

use ratatui::{Terminal, backend::TestBackend};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{FakeApp, PrBuilder, TestScenario, find_row};

fn draw(app: &FakeApp, width: u16, height: u16) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::TrueColor)))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|column| buffer[(column, row)].symbol())
        .collect()
}

#[tokio::test]
async fn header_names_lowest_open_layer_and_its_issues() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["failing", "review"])
        .pr(
            "failing",
            PrBuilder::new(41)
                .ready()
                .approved()
                .check("ci", "COMPLETED", Some("FAILURE")),
        )
        .pr("review", PrBuilder::new(42).ready().review_required())
        .current_branch("failing")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let buffer = draw(&app, 120, 40);
    let row = find_row(&buffer, "Next: #41 failing").expect("next-to-merge row");
    assert!(
        row_text(&buffer, row + 1).contains("- 1 check failing"),
        "each issue is an indented bullet under the Next line"
    );
}

#[tokio::test]
async fn next_line_colors_each_issue_and_replaces_the_count_strip_beside_the_pie() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["multi"])
        .pr(
            "multi",
            PrBuilder::new(61)
                .ready()
                .review_required()
                .check("ci", "COMPLETED", Some("FAILURE")),
        )
        .current_branch("multi")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);

    let tall = draw(&app, 120, 40);
    assert!(
        find_row(&tall, "✖ 1").is_none() && find_row(&tall, "… 0").is_none(),
        "the pie replaces the colored count strip"
    );
    let bullet_color = |needle: &str| {
        let row = find_row(&tall, needle).expect(needle);
        let text = row_text(&tall, row);
        let column = text.find(needle).expect(needle);
        tall[(text[..column].chars().count() as u16, row)].fg
    };
    assert_eq!(bullet_color("- 1 check failing"), theme.error);
    assert_eq!(bullet_color("- review required"), theme.warning);

    let compact = draw(&app, 120, 28);
    assert!(
        find_row(&compact, "✖ 1").is_some(),
        "without a pie the colored counts are shown"
    );
}

#[tokio::test]
async fn spare_header_rows_become_spacers_first_before_blocked_by_then_before_next() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["multi"])
        .pr(
            "multi",
            PrBuilder::new(62)
                .ready()
                .review_required()
                .check("ci", "COMPLETED", Some("FAILURE")),
        )
        .current_branch("multi")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let is_blank = |buffer: &ratatui::buffer::Buffer, row: u16| {
        let text = row_text(buffer, row);
        text.trim_matches(|c: char| {
            c == '│' || c.is_whitespace() || ('\u{2800}'..='\u{28ff}').contains(&c)
        })
        .is_empty()
    };

    // 8-row header = 6 text rows: identity, worktree, next, two bullets, blocked: no spacers.
    let tight = draw(&app, 120, 40);
    let blocked = find_row(&tight, "Blocked by:").unwrap();
    assert!(!is_blank(&tight, blocked - 1), "no spare row for a spacer");

    // 10-row header = 8 text rows: one spacer before Blocked by and one before Next.
    let roomy = draw(&app, 120, 44);
    let blocked = find_row(&roomy, "Blocked by:").unwrap();
    let next = find_row(&roomy, "Next: #62").unwrap();
    assert!(
        is_blank(&roomy, blocked - 1),
        "blank line before Blocked by"
    );
    assert!(is_blank(&roomy, next - 1), "blank line before Next");
}

#[tokio::test]
async fn header_marks_ready_lowest_open_layer_ready_to_merge() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["ready", "review"])
        .pr(
            "ready",
            PrBuilder::new(51).ready().checks_all_pass().approved(),
        )
        .pr("review", PrBuilder::new(52).ready().review_required())
        .current_branch("ready")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let buffer = draw(&app, 120, 40);
    let row = find_row(&buffer, "Next: #51 ready").expect("next-to-merge row");
    assert!(row_text(&buffer, row).contains("ready to merge"));
}

#[tokio::test]
async fn header_lists_only_nonzero_blocker_categories() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["failing", "review", "ready"])
        .pr(
            "failing",
            PrBuilder::new(61)
                .ready()
                .approved()
                .check("ci", "COMPLETED", Some("FAILURE")),
        )
        .pr("review", PrBuilder::new(62).ready().review_required())
        .pr(
            "ready",
            PrBuilder::new(63).ready().checks_all_pass().approved(),
        )
        .current_branch("failing")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let buffer = draw(&app, 120, 40);
    let row = row_text(
        &buffer,
        find_row(&buffer, "Blocked by:").expect("blocker breakdown row"),
    );
    assert!(row.contains("checks 1"));
    assert!(row.contains("reviews 1"));
    for category in ["threads", "rebase", "local", "draft"] {
        assert!(!row.contains(category), "unexpected {category} category");
    }
}

#[tokio::test]
async fn header_text_stops_before_the_pie_chart() {
    let branch = "feature/this-branch-name-is-deliberately-long-enough-to-force-the-header-next-to-merge-line-to-stop-before-the-pie-chart";
    let (mut app, _) = TestScenario::builder()
        .stack("main", &[branch])
        .pr(
            branch,
            PrBuilder::new(71)
                .ready()
                .approved()
                .check("ci", "COMPLETED", Some("FAILURE")),
        )
        .current_branch(branch)
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let buffer = draw(&app, 120, 40);
    let next_row = find_row(&buffer, "Next: #71").expect("next-to-merge row");
    let pie_left_edge = 120 - ((8 - 2) * 2 + 2);
    assert!(row_text(&buffer, next_row).contains('…'));
    assert!(
        (pie_left_edge..119).all(|column| {
            let symbol = buffer[(column, next_row)].symbol();
            symbol == " "
                || symbol
                    .chars()
                    .all(|character| ('\u{2800}'..='\u{28ff}').contains(&character))
        }),
        "next-to-merge text overlaps the pie chart"
    );
}

#[tokio::test]
async fn compact_header_omits_next_and_blocker_rows() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["failing"])
        .pr(
            "failing",
            PrBuilder::new(81)
                .ready()
                .approved()
                .check("ci", "COMPLETED", Some("FAILURE")),
        )
        .current_branch("failing")
        .build()
        .into_app();
    app.poll_once(Source::All).await;

    let buffer = draw(&app, 80, 24);
    assert!(find_row(&buffer, "Next:").is_none());
    assert!(find_row(&buffer, "Blocked by:").is_none());
}
