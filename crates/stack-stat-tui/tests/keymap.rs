mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use stack_stat_core::providers::ProviderAction;
use stack_stat_tui::{
    action::{Action, KEYMAP},
    app::Mode,
    message::{Effect, Source},
};
use support::{PrBuilder, TestScenario};

fn key(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
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
    assert_eq!(app.mode(), Mode::Help);
    app.handle_key(key('j'));
    assert_eq!(app.selected_branch(), Some("auth"));
    assert_eq!(app.mode(), Mode::Normal);
    app.handle_key(key('/'));
    app.handle_key(key('q'));
    assert_eq!(app.filter(), "q");
    assert_eq!(app.mode(), Mode::Filter);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.filter(), "");
    assert_eq!(app.mode(), Mode::Normal);
    assert_eq!(
        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        vec![Effect::Quit]
    );
}

#[tokio::test]
async fn navigation_detail_open_and_refresh_have_distinct_actions() {
    let (mut app, handles) = TestScenario::builder()
        .stack("main", &["auth", "api"])
        .pr("auth", PrBuilder::new(42))
        .pr("api", PrBuilder::new(43))
        .current_branch("auth")
        .build()
        .into_app();
    app.poll_once(Source::Git).await;
    app.poll_once(Source::GitHub).await;
    app.handle_key(key('G'));
    assert_eq!(app.selected_branch(), Some("api"));
    app.handle_key(key('g'));
    assert_eq!(app.selected_branch(), Some("auth"));
    assert!(!app.detail_open());
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.detail_open());
    assert_eq!(app.handle_key(key('R')), vec![Effect::Refresh(Source::All)]);
    assert_eq!(
        app.handle_key(key('o')),
        vec![Effect::Mutate(ProviderAction::OpenPr(42))]
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
        KEYMAP
            .iter()
            .any(|binding| binding.action == Action::ToggleDetail
                && binding.key.code == KeyCode::Tab)
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
    assert!(
        matches!(app.mode(), Mode::Confirm(ProviderAction::Checkout(ref name)) if name == "api")
    );
    assert!(app.handle_key(key('n')).is_empty());
    app.handle_key(key('S'));
    assert_eq!(app.mode(), Mode::Confirm(ProviderAction::SyncStack));
    assert_eq!(
        app.handle_key(key('y')),
        vec![Effect::Mutate(ProviderAction::SyncStack)]
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
            app.handle_key(key('j'));
        }
        let effects = app.handle_key(KeyEvent::new(binding.key.code, binding.key.modifiers));
        match binding.action {
            Action::Down | Action::Bottom => {
                assert_eq!(app.selected_branch(), Some("api"), "{:?}", binding.key)
            }
            Action::Up | Action::Top => {
                assert_eq!(app.selected_branch(), Some("auth"), "{:?}", binding.key)
            }
            Action::ToggleDetail => assert!(app.detail_open()),
            Action::OpenPr => assert_eq!(effects, [Effect::Mutate(ProviderAction::OpenPr(42))]),
            Action::Checkout => assert_eq!(
                app.mode(),
                Mode::Confirm(ProviderAction::Checkout("auth".into()))
            ),
            Action::Push => assert_eq!(app.mode(), Mode::Confirm(ProviderAction::PushStack)),
            Action::Sync => assert_eq!(app.mode(), Mode::Confirm(ProviderAction::SyncStack)),
            Action::ToggleDraft => assert_eq!(
                app.mode(),
                Mode::Confirm(ProviderAction::ToggleDraft {
                    number: 42,
                    make_ready: true
                })
            ),
            Action::Refresh => assert_eq!(effects, [Effect::Refresh(Source::All)]),
            Action::Filter => assert_eq!(app.mode(), Mode::Filter),
            Action::Sort => assert_eq!(app.sort(), stack_stat_tui::app::SortOrder::PrNumber),
            Action::ToggleBots => assert!(app.show_bots()),
            Action::Help => assert_eq!(app.mode(), Mode::Help),
            Action::Quit => assert_eq!(effects, [Effect::Quit]),
        }
    }
}
