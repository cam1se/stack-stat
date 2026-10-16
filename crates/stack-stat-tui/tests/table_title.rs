mod support;

use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Modifier};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{PrBuilder, TestScenario, find_body_row, find_row};

fn draw(app: &support::FakeApp, theme: &Theme) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, theme))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn column_of(buffer: &Buffer, row: u16, text: &str) -> u16 {
    let line: String = (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect();
    let byte = line.find(text).expect("text on row");
    u16::try_from(line[..byte].chars().count()).unwrap()
}

#[tokio::test]
async fn pr_title_is_the_headline_and_branch_is_dimmed() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", PrBuilder::new(42).title("Add login flow"))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, &theme);

    let headline = find_body_row(&buffer, "#42").unwrap();
    let title_x = column_of(&buffer, headline, "Add login flow");
    assert!(
        buffer[(title_x, headline)]
            .modifier
            .contains(Modifier::BOLD)
    );
    assert_ne!(buffer[(title_x, headline)].fg, theme.muted);

    let branch_x = column_of(&buffer, headline + 1, "auth");
    assert_eq!(buffer[(branch_x, headline + 1)].fg, theme.muted);
    assert!(
        !buffer[(branch_x, headline + 1)]
            .modifier
            .contains(Modifier::BOLD)
    );
}

#[tokio::test]
async fn branch_stays_the_headline_without_a_pr() {
    let (mut app, _) = TestScenario::builder()
        .single("main", "feature")
        .current_branch("feature")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, &theme);

    assert!(find_row(&buffer, "no PR yet").is_some());
    let headline = find_row(&buffer, "feature").unwrap();
    let hint = find_row(&buffer, "no PR yet").unwrap();
    assert!(headline < hint);
}

#[tokio::test]
async fn detail_header_leads_with_the_title_and_dims_the_branch() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth"])
        .pr("auth", PrBuilder::new(42).title("Add login flow"))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::All).await;
    let theme = Theme::for_depth(ColorDepth::TrueColor);
    let buffer = draw(&app, &theme);

    let header = find_row(&buffer, "PR #42").unwrap();
    let title_x = column_of(&buffer, header + 1, "Add login flow");
    assert!(
        buffer[(title_x, header + 1)]
            .modifier
            .contains(Modifier::BOLD)
    );
    let branch_x = column_of(&buffer, header + 2, "auth");
    assert_eq!(buffer[(branch_x, header + 2)].fg, theme.muted);
}
