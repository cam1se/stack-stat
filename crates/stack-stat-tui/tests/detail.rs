mod support;

use chrono::{TimeZone, Utc};
use ratatui::{Terminal, backend::TestBackend};
use stack_stat_core::{
    model::LocalSync,
    providers::{PrCheckResponse, PrResponse, ReviewResponse},
};
use stack_stat_tui::{
    message::Source,
    theme::{ColorDepth, Theme},
    ui,
};
use support::{FakeApp, PrBuilder, TestScenario, assert_row_contains, find_row};

fn draw(app: &FakeApp) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(120, 48)).unwrap();
    terminal
        .draw(|frame| ui::render(frame, app, &Theme::for_depth(ColorDepth::TrueColor)))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn detail_pr() -> PrResponse {
    let started = Utc.timestamp_opt(1_760_000_000, 0).single().unwrap();
    PrResponse {
        number: 42,
        url: "https://example.test/pr/42".into(),
        title: "Improve details".into(),
        is_draft: false,
        review_decision: Some("CHANGES_REQUESTED".into()),
        reviews: vec![
            ReviewResponse {
                author: "reviewer".into(),
                state: "CHANGES_REQUESTED".into(),
                is_bot: false,
                submitted_at: None,
            },
            ReviewResponse {
                author: "approver".into(),
                state: "APPROVED".into(),
                is_bot: false,
                submitted_at: None,
            },
        ],
        review_requests: vec![],
        checks: vec![
            PrCheckResponse {
                name: "chk-passed".into(),
                workflow: "CI".into(),
                status: "COMPLETED".into(),
                conclusion: Some("SUCCESS".into()),
                started_at: Some(started),
                completed_at: Some(started + chrono::Duration::seconds(72)),
                details_url: String::new(),
            },
            PrCheckResponse {
                name: "chk-skipped".into(),
                workflow: "CI".into(),
                status: "COMPLETED".into(),
                conclusion: Some("SKIPPED".into()),
                started_at: None,
                completed_at: None,
                details_url: String::new(),
            },
            PrCheckResponse {
                name: "chk-running".into(),
                workflow: "CI".into(),
                status: "IN_PROGRESS".into(),
                conclusion: None,
                started_at: Some(started),
                completed_at: None,
                details_url: String::new(),
            },
            PrCheckResponse {
                name: "chk-failed".into(),
                workflow: "CI".into(),
                status: "COMPLETED".into(),
                conclusion: Some("FAILURE".into()),
                started_at: Some(started),
                completed_at: Some(started + chrono::Duration::seconds(12)),
                details_url: String::new(),
            },
        ],
        unresolved_threads: 0,
    }
}

async fn app_with(pr: PrResponse) -> FakeApp {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["feature"])
        .pr("feature", PrBuilder::new(42))
        .current_branch("feature")
        .build()
        .into_app();
    handles.github.set_prs(vec![pr]);
    app.poll_once(Source::All).await;
    app.tick(Utc.timestamp_opt(1_760_000_090, 0).single().unwrap());
    app
}

#[tokio::test]
async fn detail_orders_checks_and_shows_durations() {
    let app = app_with(detail_pr()).await;
    let buffer = draw(&app);
    let rows = ["chk-failed", "chk-running", "chk-passed", "chk-skipped"]
        .map(|name| find_row(&buffer, name).expect("check should be rendered"));
    assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(find_row(&buffer, "12s").is_some());
    assert!(find_row(&buffer, "1m 12s").is_some());
    assert!(find_row(&buffer, "1m 30s").is_some());
}

#[tokio::test]
async fn detail_shows_review_headline_and_verdict_badges() {
    let no_go = draw(&app_with(detail_pr()).await);
    let header = find_row(&no_go, "PR #42").expect("detail header");
    assert_row_contains(&no_go, header, &["PR #42", "✖", "Open"]);
    assert!(find_row(&no_go, "Changes requested · 1/2 reviewers").is_some());

    let go = app_with(PrBuilder::new(42).checks_all_pass().approved().build()).await;
    let go_buffer = draw(&go);
    let header = find_row(&go_buffer, "PR #42").expect("detail header");
    assert_row_contains(&go_buffer, header, &["PR #42", "✔", "Open"]);
}

#[tokio::test]
async fn detail_identifies_the_current_branch_local_sync_state() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["feature"])
        .pr("feature", PrBuilder::new(42))
        .current_branch("feature")
        .build()
        .into_app();
    handles.git.set_current_sync(LocalSync::Ahead);
    app.poll_once(Source::All).await;

    let buffer = draw(&app);
    let header = find_row(&buffer, "PR #42").expect("detail header");
    assert_row_contains(
        &buffer,
        header,
        &["* current worktree", "local ahead of remote"],
    );
}
