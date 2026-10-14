mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stack_stat_core::providers::ProviderAction;
use stack_stat_tui::{
    action::{Action, KEYMAP},
    app::{Mode, NoticeLevel},
    message::{ActionSteps, Effect, Source},
};
use support::{PrBuilder, TestScenario};

fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
}

fn confirmation_action(app: &support::FakeApp) -> &ProviderAction {
    let Mode::Confirm(prompt) = app.mode() else {
        panic!("expected confirmation");
    };
    assert!(prompt.steps.before.is_none());
    &prompt.steps.action
}

#[tokio::test]
async fn help_blocks_navigation_and_filter_treats_q_as_text() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    assert_eq!(app.selected_branch(), Some("auth"));
    app.handle_key(key('?'));
    assert_eq!(app.mode(), &Mode::Help);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("auth"));
    assert_eq!(app.mode(), &Mode::Normal);
    app.handle_key(key('/'));
    app.handle_key(key('q'));
    assert_eq!(app.filter(), "q");
    assert_eq!(app.mode(), &Mode::Filter);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.filter(), "");
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        vec![Effect::Quit]
    );
}

#[tokio::test]
async fn navigation_refresh_and_unbound_enter_tab_have_distinct_actions() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("api"));
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("auth"));
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .is_empty()
    );
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .is_empty()
    );
    assert_eq!(app.handle_key(key('R')), vec![Effect::Refresh(Source::All)]);
    assert_eq!(
        app.handle_key(key('o')),
        vec![Effect::Mutate(ActionSteps::single(ProviderAction::OpenPr(
            42
        )))]
    );
    assert!(handles.github.recorded_actions().is_empty());
    app.handle_key(key('b'));
    assert!(app.show_bots());
    app.handle_key(key('q'));
}

#[test]
fn keymap_keeps_help_text_and_dispatch_on_the_same_entries() {
    assert!(KEYMAP.iter().all(|binding| !binding.help.is_empty()));
    assert!(
        !KEYMAP
            .iter()
            .any(|binding| matches!(binding.key.code, KeyCode::Enter | KeyCode::Tab))
    );
    assert!(KEYMAP.iter().any(|binding| binding.action == Action::Sync && binding.key.code == KeyCode::Char('S')));
    let mut keys = std::collections::HashSet::new();
    for binding in KEYMAP {
        assert!(
            keys.insert((binding.key.code, binding.key.modifiers)),
            "duplicate binding: {:?}",
            binding.key
        );
    }
}

#[test]
fn navigation_and_scrolling_actions_have_exactly_one_binding_each() {
    for action in [
        Action::Down,
        Action::Up,
        Action::Top,
        Action::Bottom,
        Action::DetailPageDown,
        Action::DetailPageUp,
    ] {
        let bindings: Vec<_> = KEYMAP
            .iter()
            .filter(|binding| binding.action == action)
            .collect();
        assert_eq!(bindings.len(), 1, "{action:?}");
        assert!(
            !matches!(bindings[0].key.code, KeyCode::Char(_)),
            "{action:?} must use a named key, not a letter alias"
        );
    }
}

#[tokio::test]
async fn checkout_and_sync_require_distinct_confirmations_and_arrows_navigate() {
    let (mut app, _) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_branch(), Some("api"));
    app.handle_key(key('c'));
    assert_eq!(
        confirmation_action(&app),
        &ProviderAction::Checkout("api".into())
    );
    assert!(app.handle_key(key('n')).is_empty());
    app.handle_key(key('S'));
    assert_eq!(confirmation_action(&app), &ProviderAction::SyncStack);
    assert_eq!(
        app.handle_key(key('y')),
        vec![Effect::Mutate(ActionSteps::single(
            ProviderAction::SyncStack
        ))]
    );
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(
        app.selected_branch(),
        Some("auth"),
        "navigation remains available while mutation runs"
    );
}

#[tokio::test]
async fn every_normal_mode_binding_has_its_documented_effect() {
    for binding in KEYMAP {
        let (mut app, _) = TestScenario::builder()
            .stack("main", &["auth", "api"])
            .pr("auth", PrBuilder::new(42).draft())
            .pr("api", PrBuilder::new(43))
            .current_branch("auth")
            .build()
            .into_app();
        app.poll_once(Source::Git).await;
        app.poll_once(Source::GitHub).await;
        if matches!(binding.action, Action::Up | Action::Top) {
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        let effects = app.handle_key(KeyEvent::new(binding.key.code, binding.key.modifiers));
        match binding.action {
            Action::Down | Action::Bottom => {
                assert_eq!(app.selected_branch(), Some("api"), "{:?}", binding.key)
            }
            Action::Up | Action::Top => {
                assert_eq!(app.selected_branch(), Some("auth"), "{:?}", binding.key)
            }
            Action::DetailPageDown | Action::DetailPageUp => {
                assert_eq!(app.selected_branch(), Some("auth"), "{:?}", binding.key);
            }
            Action::OpenPr => assert_eq!(
                effects,
                [Effect::Mutate(ActionSteps::single(ProviderAction::OpenPr(
                    42
                )))]
            ),
            Action::Checkout => assert_eq!(
                confirmation_action(&app),
                &ProviderAction::Checkout("auth".into())
            ),
            Action::Push => assert_eq!(confirmation_action(&app), &ProviderAction::PushStack),
            Action::Sync => assert_eq!(confirmation_action(&app), &ProviderAction::SyncStack),
            Action::ToggleDraft => assert_eq!(
                confirmation_action(&app),
                &ProviderAction::ToggleDraft {
                    number: 42,
                    make_ready: true,
                }
            ),
            Action::Refresh => assert_eq!(effects, [Effect::Refresh(Source::All)]),
            Action::Filter => assert_eq!(app.mode(), &Mode::Filter),
            Action::Sort => assert_eq!(app.sort(), stack_stat_tui::app::SortOrder::PrNumber),
            Action::ToggleBots => assert!(app.show_bots()),
            Action::Help => assert_eq!(app.mode(), &Mode::Help),
            Action::Quit => assert_eq!(effects, [Effect::Quit]),
        }
    }
}

#[tokio::test]
async fn single_mode_disables_checkout_and_sync_without_confirming_or_calling_provider() {
    for has_pr in [false, true] {
        let scenario = TestScenario::builder().single("main", "feature");
        let scenario = if has_pr {
            scenario.pr("feature", PrBuilder::new(42))
        } else {
            scenario
        };
        let (mut app, handles) = scenario.into_app();
        app.poll_once(Source::All).await;

        for (command, operation) in [('c', "checkout"), ('S', "sync")] {
            assert!(app.handle_key(key(command)).is_empty());
            assert_eq!(app.mode(), &Mode::Normal);
            let notice = app.notice().expect("disabled action explains why");
            assert_eq!(notice.level, NoticeLevel::Info);
            assert!(notice.text.to_lowercase().contains(operation));
            assert!(notice.text.contains("single"));
            assert!(app.handle_key(key('y')).is_empty());
        }
        assert!(handles.github.recorded_actions().is_empty());
    }
}

#[tokio::test]
async fn single_mode_without_pr_reports_pr_actions_without_provider_calls() {
    let (mut app, handles) = TestScenario::builder().single("main", "feature").into_app();
    app.poll_once(Source::All).await;

    for command in ['o', 'm'] {
        assert!(app.handle_key(key(command)).is_empty());
        assert_eq!(app.mode(), &Mode::Normal);
        let notice = app.notice().expect("missing PR is explained");
        assert_eq!(notice.level, NoticeLevel::Info);
        assert!(notice.text.contains("no PR"));
        assert!(notice.text.contains("feature"));
        assert!(app.handle_key(key('y')).is_empty());
    }
    assert!(handles.github.recorded_actions().is_empty());
}

#[tokio::test]
async fn single_mode_push_requires_confirmation_and_reports_branch_success() {
    for has_pr in [false, true] {
        let scenario = TestScenario::builder().single("main", "feature");
        let scenario = if has_pr {
            scenario.pr("feature", PrBuilder::new(42))
        } else {
            scenario
        };
        let (mut app, handles) = scenario.into_app();
        app.poll_once(Source::All).await;

        assert!(app.handle_key(key('p')).is_empty());
        let Mode::Confirm(prompt) = app.mode() else {
            panic!("plain branch push must require confirmation");
        };
        assert_eq!(prompt.steps.action, ProviderAction::PushBranch);
        assert!(prompt.steps.before.is_none());
        assert!(prompt.detail.contains("git push"));
        assert!(handles.github.recorded_actions().is_empty());
        app.handle_key(key('n'));
        assert_eq!(app.mode(), &Mode::Normal);
        assert!(handles.github.recorded_actions().is_empty());

        app.handle_key(key('p'));
        let effect = app.handle_key(key('y')).pop().expect("push is authorized");
        app.run_effect(effect).await;
        assert_eq!(
            handles.github.recorded_actions(),
            [ProviderAction::PushBranch]
        );
        assert_eq!(app.mode(), &Mode::Normal);
        let notice = app.notice().unwrap();
        assert_eq!(notice.level, NoticeLevel::Info);
        assert!(notice.text.to_lowercase().contains("branch"));
        assert!(!notice.text.to_lowercase().contains("stack"));
    }
}

#[tokio::test]
async fn single_mode_existing_pr_can_open_and_toggle_draft_both_directions() {
    let (mut app, handles) = TestScenario::builder()
        .single("main", "feature")
        .pr("feature", PrBuilder::new(42).draft())
        .into_app();
    app.poll_once(Source::All).await;

    let effect = app.handle_key(key('o')).pop().expect("existing PR opens");
    assert_eq!(app.mode(), &Mode::Normal);
    app.run_effect(effect).await;
    assert!(app.notice().unwrap().text.contains("42"));

    for make_ready in [true, false] {
        if !make_ready {
            handles
                .github
                .set_prs(vec![PrBuilder::new(42).ready().build()]);
            app.poll_once(Source::GitHub).await;
        }
        assert!(app.handle_key(key('m')).is_empty());
        assert_eq!(
            confirmation_action(&app),
            &ProviderAction::ToggleDraft {
                number: 42,
                make_ready
            }
        );
        let effect = app.handle_key(key('y')).pop().unwrap();
        app.run_effect(effect).await;
        assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);
        assert!(
            app.notice()
                .unwrap()
                .text
                .contains(if make_ready { "ready" } else { "draft" })
        );
    }
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::OpenPr(42),
            ProviderAction::ToggleDraft {
                number: 42,
                make_ready: true
            },
            ProviderAction::ToggleDraft {
                number: 42,
                make_ready: false
            },
        ]
    );
}

#[tokio::test]
async fn one_entry_real_stack_keeps_checkout_push_sync_and_local_link_recovery() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["feature"])
        .pr("feature", PrBuilder::new(42))
        .current_branch("feature")
        .into_app();
    app.poll_once(Source::All).await;

    for (command, action) in [
        ('c', ProviderAction::Checkout("feature".into())),
        ('p', ProviderAction::PushStack),
        ('S', ProviderAction::SyncStack),
    ] {
        assert!(app.handle_key(key(command)).is_empty());
        assert_eq!(confirmation_action(&app), &action);
        let effect = app.handle_key(key('y')).pop().unwrap();
        app.run_effect(effect).await;
        assert_eq!(app.mode(), &Mode::Normal);
        assert_eq!(app.notice().unwrap().level, NoticeLevel::Info);
    }
    handles.github.set_mutation_results([
        Err(stack_stat_core::providers::GitHubError::LocalTrackingMissing),
        Ok(String::new()),
        Ok(String::new()),
    ]);
    app.handle_key(key('p'));
    let effect = app.handle_key(key('y')).pop().unwrap();
    app.run_effect(effect).await;
    let Mode::Confirm(prompt) = app.mode() else {
        panic!("one-entry stack still offers explicit local linking");
    };
    assert_eq!(
        prompt.steps.before,
        Some(ProviderAction::Checkout("42".into()))
    );
    assert_eq!(prompt.steps.action, ProviderAction::PushStack);
    let effect = app.handle_key(key('y')).pop().unwrap();
    app.run_effect(effect).await;
    assert_eq!(app.mode(), &Mode::Normal);
    assert_eq!(
        handles.github.recorded_actions(),
        [
            ProviderAction::Checkout("feature".into()),
            ProviderAction::PushStack,
            ProviderAction::SyncStack,
            ProviderAction::PushStack,
            ProviderAction::Checkout("42".into()),
            ProviderAction::PushStack,
        ]
    );
}

#[tokio::test]
async fn known_single_pr_without_details_can_open_but_draft_waits_for_details() {
    let (mut app, handles) = TestScenario::builder()
        .single("main", "feature")
        .pr("feature", PrBuilder::new(42))
        .into_app();
    handles.github.set_prs(vec![]);
    app.poll_once(Source::All).await;
    assert_eq!(app.state().branches[0].pr_number, Some(42));
    assert!(app.state().branches[0].pr.is_none());

    let effect = app
        .handle_key(key('o'))
        .pop()
        .expect("known PR opens without details");
    app.run_effect(effect).await;
    assert_eq!(
        handles.github.recorded_actions(),
        [ProviderAction::OpenPr(42)]
    );
    assert!(app.handle_key(key('m')).is_empty());
    assert_eq!(app.mode(), &Mode::Normal);
    let notice = app.notice().expect("missing details are explained");
    assert_eq!(notice.level, NoticeLevel::Info);
    assert!(notice.text.contains("details"));
    assert!(!notice.text.contains("no PR"));
    assert_eq!(
        handles.github.recorded_actions(),
        [ProviderAction::OpenPr(42)]
    );
}
