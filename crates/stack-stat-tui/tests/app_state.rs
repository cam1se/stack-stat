mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stack_stat_core::{
    model::{LocalCheckState, StackStatus},
    providers::{CheckResult, GitHubError, ProviderAction, ReportedState, StatusStoreProvider},
};
use stack_stat_tui::{
    app::{Mode, NoticeLevel, SortOrder},
    message::{ActionSteps, Effect, Source},
};
use support::{PrBuilder, TestScenario};

fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}
fn scenario() -> (support::FakeApp, support::Handles) {
    TestScenario::builder()
        .stack("main", &["auth", "api", "ui"])
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
                .check("ci/build", "COMPLETED", Some("FAILURE")),
        )
        .current_branch("auth")
        .build()
        .into_app()
}

#[tokio::test]
async fn agent_branch_switch_moves_current_row_after_git_poll() {
    let (mut app, handles) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    assert!(app.state().branches[0].is_current);
    handles.git.set_current_branch("api");
    app.poll_once(Source::Git).await;
    assert!(!app.state().branches[0].is_current);
    assert!(app.state().branches[1].is_current);
}

#[tokio::test]
async fn initial_selection_prefers_worktree_branch_and_falls_back_to_first_visible() {
    let (mut selected_current, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("api")
        .build()
        .into_app();
    selected_current.poll_once(Source::All).await;
    assert_eq!(selected_current.selected_branch(), Some("api"));

    let (mut no_matching_current, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("outside")
        .build()
        .into_app();
    no_matching_current.poll_once(Source::All).await;
    assert_eq!(no_matching_current.selected_branch(), Some("auth"));
}

#[tokio::test]
async fn initial_selection_moves_to_worktree_when_git_reports_after_github() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api", "ui"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .pr("ui", PrBuilder::new(44))
        .current_branch("api")
        .build()
        .into_app();
    app.poll_once(Source::GitHub).await;
    app.poll_once(Source::Git).await;
    assert_eq!(app.selected_branch(), Some("api"));

    // Once anchored, later worktree switches leave the selection alone.
    handles.git.set_current_branch("ui");
    app.poll_once(Source::Git).await;
    assert_eq!(app.selected_branch(), Some("api"));
}

#[tokio::test]
async fn explicit_selection_before_git_reports_is_kept() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api", "ui"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .pr("ui", PrBuilder::new(44))
        .current_branch("api")
        .build()
        .into_app();
    app.poll_once(Source::GitHub).await;
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    app.poll_once(Source::Git).await;
    assert_eq!(app.selected_branch(), Some("ui"));
}

#[tokio::test]
async fn draft_toggle_requires_confirmation_and_records_effect() {
    let (mut app, handles) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("api"));
    assert!(app.handle_key(key('m')).is_empty());
    assert!(matches!(
        app.mode(),
        Mode::Confirm(prompt) if prompt.steps.action == ProviderAction::ToggleDraft {
            number: 43,
            make_ready: true,
        }
    ));
    let effects = app.handle_key(key('y'));
    assert_eq!(effects.len(), 1);
    app.run_effect(effects.into_iter().next().unwrap()).await;
    assert!(
        handles
            .github
            .recorded_actions()
            .contains(&ProviderAction::ToggleDraft {
                number: 43,
                make_ready: true
            })
    );
    assert!(app.notice().unwrap().text.contains("ready"));
}

#[tokio::test]
async fn off_stack_replaces_rows_but_network_failure_retains_last_good_rows() {
    let (mut app, handles) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    handles
        .github
        .set_stack_error(GitHubError::Network("connection refused".into()));
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().branches.len(), 3);
    assert!(app.notice().unwrap().text.contains("connection refused"));
    handles.git.set_current_branch("api");
    app.poll_once(Source::Git).await;
    assert!(app.state().branches[1].is_current);
    handles
        .store
        .write(
            "api",
            &format!("{:040x}", 2),
            "lint",
            &CheckResult {
                state: ReportedState::Pass,
                message: None,
                recorded_at: chrono::DateTime::from_timestamp(1_760_000_000, 0).unwrap(),
            },
        )
        .unwrap();
    app.poll_once(Source::Store).await;
    assert_eq!(
        app.state().branches[1].local_checks[0].state,
        LocalCheckState::Pass
    );
    assert!(
        app.notice().unwrap().text.contains("connection refused"),
        "local refresh must not hide GitHub failure"
    );
    handles.github.set_stack_error(GitHubError::NotInStack);
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().stack_status, StackStatus::NotInStack);
    assert!(app.state().branches.is_empty());
}

#[tokio::test]
async fn selection_survives_refresh_and_filter_sort_and_cancel_are_predictable() {
    let (mut app, _handles) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.selected_branch(), Some("api"));
    app.handle_key(key('/'));
    app.handle_key(key('u'));
    app.handle_key(key('i'));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("ui"));
    assert_eq!(app.filter(), "ui");
    app.handle_key(key('s'));
    assert_eq!(app.sort(), SortOrder::PrNumber);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.handle_key(key('p'));
    assert!(matches!(
        app.mode(),
        Mode::Confirm(prompt) if prompt.steps.action == ProviderAction::PushStack
    ));
    assert!(app.handle_key(key('n')).is_empty());
    assert_eq!(app.mode(), &Mode::Normal);
    assert!(app.notice().unwrap().text.contains("cancelled"));
}

#[tokio::test]
async fn busy_mutation_and_pr_less_row_do_not_launch_commands() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth"])
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    assert!(app.handle_key(key('o')).is_empty());
    assert!(app.notice().unwrap().text.contains("no PR"));
    assert!(app.handle_key(key('m')).is_empty());
    assert!(app.handle_key(key('p')).is_empty());
    let effects = app.handle_key(key('y'));
    assert!(matches!(
        effects.as_slice(),
        [Effect::Mutate(ActionSteps {
            before: None,
            action: ProviderAction::PushStack
        })]
    ));
    assert!(app.handle_key(key('c')).is_empty());
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Progress);
    assert!(handles.github.recorded_actions().is_empty());
}

#[test]
fn active_checks_shorten_github_interval_and_rate_limit_reset_is_bounded() {
    use chrono::{TimeZone, Utc};
    use stack_stat_core::config::PollConfig;
    use stack_stat_tui::poll::{github_interval, rate_limit_wait};
    use std::time::Duration;

    let intervals = PollConfig::default();
    assert_eq!(github_interval(&intervals, false), Duration::from_secs(30));
    assert_eq!(github_interval(&intervals, true), Duration::from_secs(15));
    let now = Utc.with_ymd_and_hms(2026, 10, 12, 12, 0, 0).unwrap();
    assert_eq!(
        rate_limit_wait(Some("resets at 2026-10-12T12:02:00Z"), now),
        Duration::from_secs(120)
    );
    assert_eq!(rate_limit_wait(None, now), Duration::from_secs(300));
}

#[tokio::test]
async fn failed_mutation_persists_and_new_remote_failure_replaces_it() {
    let (mut app, _) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.apply(stack_stat_tui::message::AppMessage::MutationDone {
        steps: ActionSteps::single(ProviderAction::PushStack),
        result: Err(GitHubError::Network("push denied".into())),
    });
    assert!(app.notice().unwrap().text.contains("push denied"));
    app.poll_once(Source::Git).await;
    assert!(
        app.notice().unwrap().text.contains("push denied"),
        "source refresh cannot clear a mutation failure"
    );
    app.apply(stack_stat_tui::message::AppMessage::GitHub(Err(
        GitHubError::Network("remote offline".into()),
    )));
    app.poll_once(Source::Git).await;
    assert!(app.notice().unwrap().text.contains("remote offline"));
}

#[tokio::test]
async fn confirmation_prompt_survives_unrelated_local_refresh() {
    let (mut app, _) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.apply(stack_stat_tui::message::AppMessage::GitHub(Err(
        GitHubError::Network("remote offline".into()),
    )));
    app.handle_key(key('p'));
    assert!(
        matches!(app.mode(), Mode::Confirm(prompt) if prompt.steps.action == ProviderAction::PushStack)
    );
    app.poll_once(Source::Git).await;
    assert!(app.notice().unwrap().text.contains("remote offline"));
    assert!(
        matches!(app.mode(), Mode::Confirm(prompt) if prompt.steps.action == ProviderAction::PushStack)
    );
}

#[tokio::test]
async fn open_pr_completion_cannot_unlock_an_in_flight_push() {
    let (mut app, _) = scenario();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.handle_key(key('p'));
    assert_eq!(
        app.handle_key(key('y')),
        [Effect::Mutate(ActionSteps::single(
            ProviderAction::PushStack
        ))]
    );
    assert_eq!(
        app.handle_key(key('o')),
        [Effect::Mutate(ActionSteps::single(ProviderAction::OpenPr(
            42
        )))]
    );
    app.apply(stack_stat_tui::message::AppMessage::MutationDone {
        steps: ActionSteps::single(ProviderAction::OpenPr(42)),
        result: Ok(String::new()),
    });
    assert!(app.handle_key(key('S')).is_empty());
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Progress);
}
