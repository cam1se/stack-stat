mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stack_stat_core::{
    model::{Issue, LocalCheckState, LocalSync, RebaseNeed, StackMode, StackStatus, Verdict},
    providers::{
        CheckResult, GitHubError, ProviderAction, ReportedState, StackBranch, StackPr,
        StackViewResponse, StatusStoreProvider,
    },
};
use stack_stat_tui::{
    app::{Mode, NoticeLevel, SortOrder},
    message::{ActionSteps, Effect, Source},
};
use support::{ConfigBuilder, PrBuilder, TestScenario};

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
    assert!(app.handle_key(key('d')).is_empty());
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
    app.handle_key(key('t'));
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
    assert!(app.handle_key(key('d')).is_empty());
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

#[tokio::test]
async fn single_pr_poll_exposes_remote_readiness_and_local_worktree_evidence() {
    let (mut app, handles) = TestScenario::builder()
        .single("develop", "feature")
        .pr(
            "feature",
            PrBuilder::new(42)
                .title("Ordinary pull request")
                .draft()
                .check("ci/build", "COMPLETED", Some("FAILURE"))
                .check("ci/test", "IN_PROGRESS", None)
                .changes_requested()
                .unresolved_threads(2),
        )
        .remote_rebase("feature")
        .repo("acme/widget")
        .config(ConfigBuilder::new().gate_on_unresolved_threads(true))
        .into_app();
    handles.git.set_current_sync(LocalSync::Ahead);
    app.poll_once(Source::All).await;

    assert_eq!(app.state().mode, StackMode::Single);
    assert_eq!(app.state().stack_status, StackStatus::Ready);
    assert_eq!(app.state().trunk, "develop");
    assert_eq!(app.state().repo, "acme/widget");
    assert_eq!(app.selected_branch(), Some("feature"));
    let branch = &app.state().branches[0];
    let pr = branch.pr.as_ref().expect("single PR details are composed");
    assert_eq!(pr.title, "Ordinary pull request");
    assert_eq!(pr.checks.len(), 2);
    assert_eq!(pr.reviews.len(), 1);
    assert!(branch.is_current);
    assert_eq!(branch.local_sync, LocalSync::Ahead);
    assert_eq!(branch.rebase, RebaseNeed::Trunk);
    for issue in [
        Issue::Draft,
        Issue::FailingChecks(1),
        Issue::RunningChecks(1),
        Issue::ChangesRequested,
        Issue::UnresolvedThreads(2),
    ] {
        assert!(branch.issues().contains(&issue), "missing {issue:?}");
    }
    assert_eq!(branch.verdict(), Verdict::NoGo);
    assert_eq!(app.state().next_to_merge().unwrap().name, "feature");
    assert!(app.ci_active());
}

#[tokio::test]
async fn no_pr_poll_keeps_failed_stale_and_missing_local_checks_without_merge_candidate() {
    let (mut app, handles) = TestScenario::builder()
        .single("main", "feature")
        .config(
            ConfigBuilder::new()
                .local("lint")
                .local("unit")
                .local("build"),
        )
        .local_check(
            "feature",
            &format!("{:040x}", 1),
            "lint",
            ReportedState::Fail,
        )
        .local_check(
            "feature",
            &format!("{:040x}", 1),
            "unit",
            ReportedState::Pass,
        )
        .into_app();
    handles.git.set_current_sync(LocalSync::Unpushed);
    app.poll_once(Source::All).await;

    assert_eq!(app.state().mode, StackMode::Single);
    assert_eq!(app.state().stack_status, StackStatus::Ready);
    assert_eq!(app.selected_branch(), Some("feature"));
    let branch = &app.state().branches[0];
    assert!(branch.pr_number.is_none());
    assert!(branch.pr.is_none());
    assert_eq!(branch.local_sync, LocalSync::Unpushed);
    let lint = branch
        .local_checks
        .iter()
        .find(|check| check.name == "lint")
        .unwrap();
    assert_eq!(lint.state, LocalCheckState::Fail);
    assert!(!lint.is_stale);
    let unit = branch
        .local_checks
        .iter()
        .find(|check| check.name == "unit")
        .unwrap();
    assert_eq!(unit.state, LocalCheckState::Pass);
    assert!(!unit.is_stale);
    let build = branch
        .local_checks
        .iter()
        .find(|check| check.name == "build")
        .unwrap();
    assert_eq!(build.state, LocalCheckState::Missing);
    for issue in [Issue::NoPr, Issue::LocalFailed(1), Issue::MissingLocal(1)] {
        assert!(branch.issues().contains(&issue), "missing {issue:?}");
    }
    assert!(!branch.issues().contains(&Issue::LoadingDetails));
    assert_ne!(branch.verdict(), Verdict::Go);
    assert!(app.state().next_to_merge().is_none());
    assert!(!app.ci_active());

    handles
        .git
        .set_branch_sha("feature", &format!("{:040x}", 2));
    app.poll_once(Source::Git).await;
    let branch = &app.state().branches[0];
    for name in ["lint", "unit"] {
        assert!(
            branch
                .local_checks
                .iter()
                .find(|check| check.name == name)
                .unwrap()
                .is_stale
        );
    }
    for issue in [
        Issue::NoPr,
        Issue::LocalFailed(1),
        Issue::LocalStale(2),
        Issue::MissingLocal(1),
    ] {
        assert!(branch.issues().contains(&issue), "missing {issue:?}");
    }
    assert!(app.state().next_to_merge().is_none());
}

#[tokio::test]
async fn refresh_changes_modes_and_no_pr_replaces_rows_while_failures_keep_last_snapshot() {
    let (mut app, handles) = scenario();
    app.poll_once(Source::All).await;
    assert_eq!(app.state().mode, StackMode::Stacked);
    assert_eq!(app.state().branches.len(), 3);

    let mut view = StackViewResponse {
        mode: StackMode::Single,
        trunk: "develop".into(),
        current_branch: "feature".into(),
        branches: vec![StackBranch {
            name: "feature".into(),
            is_merged: false,
            is_queued: false,
            pr: Some(StackPr {
                number: 45,
                url: "https://example.test/pr/45".into(),
                state: "OPEN".into(),
            }),
        }],
    };
    handles.git.set_current_branch("feature");
    let sha = format!("{:040x}", 4);
    handles.git.set_branch_sha("feature", &sha);
    handles
        .store
        .write(
            "feature",
            &sha,
            "lint",
            &CheckResult {
                state: ReportedState::Fail,
                message: None,
                recorded_at: chrono::DateTime::from_timestamp(1_760_000_000, 0).unwrap(),
            },
        )
        .unwrap();
    handles.github.set_stack(view.clone());
    handles.github.set_prs(vec![
        PrBuilder::new(45).checks_all_pass().approved().build(),
    ]);
    handles
        .github
        .set_remote_rebase_branches(["feature".into()]);
    app.poll_once(Source::All).await;
    assert_eq!(app.state().mode, StackMode::Single);
    assert_eq!(app.state().branches.len(), 1);
    assert_eq!(app.selected_branch(), Some("feature"));
    assert_eq!(app.state().branches[0].pr_number, Some(45));
    assert!(app.state().branches[0].pr.is_some());
    assert_eq!(app.state().next_to_merge().unwrap().name, "feature");
    assert_eq!(app.state().branches[0].rebase, RebaseNeed::Trunk);
    let locals = app.state().branches[0].local_checks.clone();
    assert_eq!(locals[0].state, LocalCheckState::Fail);

    handles
        .github
        .set_stack_error(GitHubError::Network("remote unavailable".into()));
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().mode, StackMode::Single);
    assert_eq!(app.state().branches[0].pr_number, Some(45));
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    app.handle_key(key('p'));
    assert!(
        matches!(app.mode(), Mode::Confirm(prompt) if prompt.steps.action == ProviderAction::PushBranch)
    );
    app.handle_key(key('n'));

    view.branches[0].pr = None;
    handles.github.set_stack(view.clone());
    app.poll_once(Source::GitHub).await;
    assert!(app.notice().is_none());
    assert_eq!(app.state().mode, StackMode::Single);
    assert_eq!(app.state().branches.len(), 1);
    assert!(app.state().branches[0].pr_number.is_none());
    assert!(app.state().branches[0].pr.is_none());
    assert_eq!(app.state().branches[0].rebase, RebaseNeed::None);
    assert_eq!(app.state().branches[0].local_checks, locals);
    assert!(app.state().next_to_merge().is_none());
    assert_eq!(app.selected_branch(), Some("feature"));

    handles.github.set_stack_error(GitHubError::Auth);
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().mode, StackMode::Single);
    assert!(app.state().branches[0].pr_number.is_none());
    assert!(app.state().branches[0].pr.is_none());
    assert!(app.state().next_to_merge().is_none());

    view.mode = StackMode::Stacked;
    handles.github.set_stack(view);
    app.poll_once(Source::GitHub).await;
    assert!(app.notice().is_none());
    assert_eq!(app.state().mode, StackMode::Stacked);
    app.handle_key(key('p'));
    assert!(
        matches!(app.mode(), Mode::Confirm(prompt) if prompt.steps.action == ProviderAction::PushStack)
    );
}
