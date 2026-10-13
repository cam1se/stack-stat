mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stack_stat_core::providers::ProviderAction;
use stack_stat_tui::{
    action::{Action, KEYMAP},
    app::Mode,
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
