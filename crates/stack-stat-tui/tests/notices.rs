mod support;

use chrono::{DateTime, Duration, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stack_stat_core::{
    compose::StoreSnapshot,
    model::StackMode,
    providers::{GitHubError, ProviderAction, StackBranch, StackPr, StackViewResponse},
};
use stack_stat_tui::{
    app::{Mode, NoticeLevel, Prompt},
    message::{ActionSteps, AppMessage, Effect, Source},
};
use support::{FakeApp, Handles, PrBuilder, TestScenario};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn clock() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 13, 12, 0, 0).unwrap()
}

async fn dashboard() -> (FakeApp, Handles) {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("api")
        .build()
        .into_app();
    app.tick(clock());
    app.poll_once(Source::All).await;
    (app, handles)
}

fn confirm(app: &mut FakeApp, command: char) -> Effect {
    assert!(app.handle_key(key(KeyCode::Char(command))).is_empty());
    assert!(matches!(app.mode(), Mode::Confirm(_)));
    app.handle_key(key(KeyCode::Char('y')))
        .pop()
        .expect("confirmation emits an action")
}

fn prompt(app: &FakeApp) -> &Prompt {
    let Mode::Confirm(prompt) = app.mode() else {
        panic!("expected a confirmation prompt");
    };
    prompt
}

fn complete(app: &mut FakeApp, action: ProviderAction, result: Result<String, GitHubError>) {
    app.apply(AppMessage::MutationDone {
        steps: ActionSteps::single(action),
        result,
    });
}

#[tokio::test]
async fn progress_has_no_expiry_and_success_info_expires_after_four_seconds() {
    let (mut app, _) = dashboard().await;
    let effect = confirm(&mut app, 'p');
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Progress);
    assert_eq!(app.notice().unwrap().expires_at, None);
    let completed_at = clock() + Duration::hours(1);
    app.tick(completed_at);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Progress);

    app.run_effect(effect).await;
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);
    assert_eq!(
        app.notice().unwrap().expires_at,
        Some(completed_at + Duration::seconds(4))
    );
    app.tick(completed_at + Duration::seconds(3));
    assert!(app.notice().is_some());
    app.tick(completed_at + Duration::seconds(4));
    assert!(app.notice().is_none());
}

#[tokio::test]
async fn warning_expires_after_eight_seconds() {
    let (mut app, _) = dashboard().await;
    app.apply(AppMessage::Store(Ok(StoreSnapshot {
        entries: vec![],
        warnings: vec!["one local status file is unreadable".into()],
        fetched_at: clock(),
    })));
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Warn);
    assert_eq!(
        app.notice().unwrap().expires_at,
        Some(clock() + Duration::seconds(8))
    );
    app.tick(clock() + Duration::seconds(7));
    assert!(app.notice().is_some());
    app.tick(clock() + Duration::seconds(8));
    assert!(app.notice().is_none());
}

#[tokio::test]
async fn mutation_error_persists_across_time_and_source_refresh_until_escape() {
    let (mut app, _) = dashboard().await;
    complete(
        &mut app,
        ProviderAction::PushStack,
        Err(GitHubError::Network("push denied".into())),
    );
    app.tick(clock() + Duration::days(1));
    app.poll_once(Source::All).await;
    let notice = app.notice().unwrap();
    assert_eq!(notice.level, NoticeLevel::Error);
    assert_eq!(notice.expires_at, None);
    assert!(notice.text.contains("push denied"));

    assert!(app.handle_key(key(KeyCode::Esc)).is_empty());
    assert_eq!(app.mode(), &Mode::Normal);
    assert!(app.notice().is_none());
}

#[tokio::test]
async fn source_error_only_clears_when_its_source_recovers() {
    let (mut app, _) = dashboard().await;
    app.apply(AppMessage::GitHub(Err(GitHubError::Network(
        "remote offline".into(),
    ))));
    app.poll_once(Source::Git).await;
    assert!(app.notice().unwrap().text.contains("remote offline"));
    app.poll_once(Source::GitHub).await;
    assert!(app.notice().is_none());
}

#[tokio::test]
async fn info_and_warning_cannot_replace_error_but_a_new_error_can() {
    let (mut app, _) = dashboard().await;
    complete(
        &mut app,
        ProviderAction::PushStack,
        Err(GitHubError::Network("push denied".into())),
    );
    complete(&mut app, ProviderAction::OpenPr(43), Ok(String::new()));
    app.apply(AppMessage::Store(Ok(StoreSnapshot {
        entries: vec![],
        warnings: vec!["stale local status".into()],
        fetched_at: clock(),
    })));
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(app.notice().unwrap().text.contains("push denied"));

    complete(&mut app, ProviderAction::PushStack, Err(GitHubError::Auth));
    assert!(app.notice().unwrap().text.contains("authentication"));
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
}

#[tokio::test]
async fn confirmation_is_separate_from_notice_and_ignores_unrelated_keys() {
    let (mut app, _) = dashboard().await;
    app.apply(AppMessage::GitHub(Err(GitHubError::Network(
        "remote offline".into(),
    ))));
    let notice = app.notice().cloned();
    assert!(app.handle_key(key(KeyCode::Char('p'))).is_empty());
    assert!(prompt(&app).title.contains("Push"));
    assert_eq!(prompt(&app).steps.before, None);
    assert_eq!(prompt(&app).steps.action, ProviderAction::PushStack);
    assert_eq!(app.notice(), notice.as_ref());
    assert!(app.handle_key(key(KeyCode::Char('x'))).is_empty());
    app.poll_once(Source::Git).await;
    assert!(matches!(app.mode(), Mode::Confirm(_)));
    assert_eq!(app.notice(), notice.as_ref());
}

#[tokio::test]
async fn declining_a_confirmation_is_an_expiring_info_without_provider_calls() {
    for cancel in [KeyCode::Char('n'), KeyCode::Esc] {
        let (mut app, handles) = dashboard().await;
        app.handle_key(key(KeyCode::Char('p')));
        assert!(app.handle_key(key(cancel)).is_empty());
        assert_eq!(app.mode(), &Mode::Normal);
        assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);
        assert!(app.notice().unwrap().text.contains("cancelled"));
        assert!(handles.github.recorded_actions().is_empty());
        app.tick(clock() + Duration::seconds(4));
        assert!(app.notice().is_none());
    }
}

#[tokio::test]
async fn missing_local_tracking_prompts_before_linking_current_branch_and_retrying_push() {
    let (mut app, handles) = dashboard().await;
    handles.github.set_mutation_results([
        Err(GitHubError::LocalTrackingMissing),
        Ok(String::new()),
        Ok(String::new()),
    ]);
    // Table selection is deliberately not the worktree branch that push acts on.
    app.handle_key(key(KeyCode::Home));
    assert_eq!(app.selected_branch(), Some("auth"));
    let effect = confirm(&mut app, 'p');
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [ProviderAction::PushStack]
    );
    assert_eq!(
        prompt(&app).steps.before,
        Some(ProviderAction::Checkout("43".into()))
    );
    assert_eq!(prompt(&app).steps.action, ProviderAction::PushStack);
    app.poll_once(Source::All).await;
    assert!(matches!(app.mode(), Mode::Confirm(_)));

    let effect = app.handle_key(key(KeyCode::Char('y'))).pop().unwrap();
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::PushStack,
            ProviderAction::Checkout("43".into()),
            ProviderAction::PushStack,
        ]
    );
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);
    assert!(app.notice().unwrap().text.contains("Pushed"));
}

#[tokio::test]
async fn declining_link_prompt_does_not_checkout_or_retry_sync() {
    for cancel in [KeyCode::Char('n'), KeyCode::Esc] {
        let (mut app, handles) = dashboard().await;
        handles
            .github
            .set_mutation_results([Err(GitHubError::LocalTrackingMissing)]);
        let effect = confirm(&mut app, 'S');
        app.run_effect(effect).await;
        assert_eq!(prompt(&app).steps.action, ProviderAction::SyncStack);
        assert!(app.handle_key(key(cancel)).is_empty());
        assert_eq!(app.mode(), &Mode::Normal);
        assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);
        assert!(app.notice().unwrap().text.contains("cancelled"));
        assert_eq!(
            handles.github.recorded_actions(),
            [ProviderAction::SyncStack]
        );
    }
}

#[tokio::test]
async fn rebase_conflict_reports_resolution_commands_and_keeps_the_dashboard_open() {
    let (mut app, handles) = dashboard().await;
    handles
        .github
        .set_mutation_results([Err(GitHubError::RebaseConflict)]);
    let effect = confirm(&mut app, 'R');
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [ProviderAction::RebaseStack]
    );
    assert_eq!(app.mode(), &Mode::Normal);
    let notice = app.notice().unwrap();
    assert_eq!(notice.level, NoticeLevel::Error);
    assert!(notice.text.contains("rebase --continue"));
}

#[tokio::test]
async fn missing_local_tracking_prompts_to_link_then_retries_the_rebase() {
    let (mut app, handles) = dashboard().await;
    handles
        .github
        .set_mutation_results([Err(GitHubError::LocalTrackingMissing)]);
    let effect = confirm(&mut app, 'R');
    app.run_effect(effect).await;
    assert!(prompt(&app).title.contains("rebase"));
    let effect = app.handle_key(key(KeyCode::Char('y'))).pop().unwrap();
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::RebaseStack,
            ProviderAction::Checkout("43".into()),
            ProviderAction::RebaseStack,
        ]
    );
    assert_eq!(app.mode(), &Mode::Normal);
    assert!(app.notice().unwrap().text.contains("Rebased"));
}

#[tokio::test]
async fn failed_link_checkout_aborts_before_retry_and_leaves_persistent_error() {
    let (mut app, handles) = dashboard().await;
    handles.github.set_mutation_results([
        Err(GitHubError::LocalTrackingMissing),
        Err(GitHubError::Network("checkout failed".into())),
    ]);
    let effect = confirm(&mut app, 'S');
    app.run_effect(effect).await;
    let effect = app.handle_key(key(KeyCode::Char('y'))).pop().unwrap();
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::SyncStack,
            ProviderAction::Checkout("43".into()),
        ]
    );
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(app.notice().unwrap().text.contains("checkout failed"));
    app.tick(clock() + Duration::hours(1));
    assert!(app.notice().is_some());
}

#[tokio::test]
async fn failed_retry_after_link_does_not_prompt_or_loop_again() {
    let (mut app, handles) = dashboard().await;
    handles.github.set_mutation_results([
        Err(GitHubError::LocalTrackingMissing),
        Ok(String::new()),
        Err(GitHubError::LocalTrackingMissing),
    ]);
    let effect = confirm(&mut app, 'S');
    app.run_effect(effect).await;
    let effect = app.handle_key(key(KeyCode::Char('y'))).pop().unwrap();
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::SyncStack,
            ProviderAction::Checkout("43".into()),
            ProviderAction::SyncStack,
        ]
    );
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
}

#[tokio::test]
async fn stale_stack_does_not_link_a_different_current_branch() {
    let (mut app, handles) = dashboard().await;
    handles.git.set_current_branch("outside");
    app.poll_once(Source::Git).await;
    complete(
        &mut app,
        ProviderAction::PushStack,
        Err(GitHubError::LocalTrackingMissing),
    );
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(handles.github.recorded_actions().is_empty());
}

#[tokio::test]
async fn missing_action_extension_surfaces_install_hint_without_link_prompt() {
    let (mut app, handles) = dashboard().await;
    handles
        .github
        .set_mutation_results([Err(GitHubError::MissingStackExtension)]);
    let effect = confirm(&mut app, 'p');
    app.run_effect(effect).await;
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(
        app.notice()
            .unwrap()
            .text
            .contains("gh extension install github/gh-stack")
    );
    app.poll_once(Source::All).await;
    assert!(app.notice().is_some());
}

#[tokio::test]
async fn missing_extension_on_every_action_is_a_persistent_error_without_link_prompt() {
    for command in ['p', 'S', 'c'] {
        let (mut app, handles) = dashboard().await;
        handles
            .github
            .set_mutation_results([Err(GitHubError::MissingStackExtension)]);
        let effect = confirm(&mut app, command);
        app.run_effect(effect).await;
        assert_eq!(app.mode(), &Mode::Normal, "command {command}");
        let notice = app.notice().expect("failure is reported");
        assert_eq!(notice.level, NoticeLevel::Error);
        assert!(notice.text.contains("gh extension install github/gh-stack"));
        assert_eq!(handles.github.recorded_actions().len(), 1);
        app.tick(clock() + Duration::hours(1));
        assert!(app.notice().is_some(), "error does not expire");
    }
}

#[tokio::test]
async fn missing_extension_during_link_checkout_aborts_without_retry() {
    let (mut app, handles) = dashboard().await;
    handles.github.set_mutation_results([
        Err(GitHubError::LocalTrackingMissing),
        Err(GitHubError::MissingStackExtension),
    ]);
    let effect = confirm(&mut app, 'S');
    app.run_effect(effect).await;
    let effect = app.handle_key(key(KeyCode::Char('y'))).pop().unwrap();
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::SyncStack,
            ProviderAction::Checkout("43".into()),
        ]
    );
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(
        app.notice()
            .unwrap()
            .text
            .contains("gh extension install github/gh-stack")
    );
}

#[tokio::test]
async fn missing_extension_error_is_dismissed_by_escape_and_not_replaced_by_info() {
    let (mut app, handles) = dashboard().await;
    handles
        .github
        .set_mutation_results([Err(GitHubError::MissingStackExtension), Ok(String::new())]);
    let effect = confirm(&mut app, 'p');
    app.run_effect(effect).await;
    let effect = confirm(&mut app, 'p');
    app.run_effect(effect).await;
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);

    let effect = confirm(&mut app, 'p');
    handles
        .github
        .set_mutation_results([Err(GitHubError::MissingStackExtension)]);
    app.run_effect(effect).await;
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    complete(&mut app, ProviderAction::OpenPr(42), Ok("Opened".into()));
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(app.handle_key(key(KeyCode::Esc)).is_empty());
    assert!(app.notice().is_none());
}

#[tokio::test]
async fn modified_confirmation_keys_do_not_authorize_actions() {
    let (mut app, handles) = dashboard().await;
    app.handle_key(key(KeyCode::Char('p')));
    for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Char('y'), modifier))
                .is_empty()
        );
        assert!(matches!(app.mode(), Mode::Confirm(_)));
    }
    assert!(handles.github.recorded_actions().is_empty());
    assert_eq!(
        app.handle_key(key(KeyCode::Char('y'))),
        [Effect::Mutate(ActionSteps::single(
            ProviderAction::PushStack
        ))]
    );
}

#[tokio::test]
async fn single_branch_push_errors_remain_errors_without_offering_stack_linking() {
    for error in [
        GitHubError::Command {
            code: 128,
            stderr: "fatal: The current branch feature has no upstream branch".into(),
        },
        GitHubError::Git("fatal: not a tracked stack branch".into()),
        GitHubError::Network("push transport disconnected".into()),
        GitHubError::LocalTrackingMissing,
    ] {
        let (mut app, handles) = TestScenario::builder().single("main", "feature").into_app();
        app.tick(clock());
        app.poll_once(Source::All).await;
        handles.github.set_mutation_error(error.clone());
        let effect = confirm(&mut app, 'p');
        app.run_effect(effect).await;

        assert_eq!(app.mode(), &Mode::Normal);
        assert_eq!(
            handles.github.recorded_actions(),
            [ProviderAction::PushBranch]
        );
        let notice = app.notice().expect("push error is visible");
        assert_eq!(notice.level, NoticeLevel::Error);
        assert!(notice.text.contains(error.to_string().as_str()));
        app.tick(clock() + Duration::hours(1));
        app.poll_once(Source::All).await;
        assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
        assert_eq!(app.mode(), &Mode::Normal);
        assert!(app.handle_key(key(KeyCode::Char('y'))).is_empty());
        assert_eq!(
            handles.github.recorded_actions(),
            [ProviderAction::PushBranch]
        );
    }
}

#[tokio::test]
async fn ordinary_stacked_push_error_does_not_infer_missing_local_tracking() {
    let (mut app, handles) = dashboard().await;
    handles.github.set_mutation_error(GitHubError::Command {
        code: 128,
        stderr: "fatal: upstream unavailable; local stack may not be initialized".into(),
    });
    let effect = confirm(&mut app, 'p');
    app.run_effect(effect).await;

    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert!(app.notice().unwrap().text.contains("upstream unavailable"));
    assert_eq!(
        handles.github.recorded_actions(),
        [ProviderAction::PushStack]
    );
    assert!(app.handle_key(key(KeyCode::Char('y'))).is_empty());
}

#[tokio::test]
async fn old_stacked_push_failure_cannot_offer_linking_after_refresh_to_single_mode() {
    let (mut app, handles) = dashboard().await;
    let effect = confirm(&mut app, 'p');
    handles.github.set_stack(StackViewResponse {
        mode: StackMode::Single,
        trunk: "main".into(),
        current_branch: "api".into(),
        branches: vec![StackBranch {
            name: "api".into(),
            is_merged: false,
            is_queued: false,
            pr: Some(StackPr {
                number: 43,
                url: "https://example.test/pr/43".into(),
                state: "OPEN".into(),
            }),
        }],
    });
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.state().mode, StackMode::Single);
    handles
        .github
        .set_mutation_error(GitHubError::LocalTrackingMissing);
    app.run_effect(effect).await;

    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(app.notice().unwrap().level, NoticeLevel::Error);
    assert_eq!(
        handles.github.recorded_actions(),
        [ProviderAction::PushStack]
    );
    assert!(app.handle_key(key(KeyCode::Char('y'))).is_empty());
}
