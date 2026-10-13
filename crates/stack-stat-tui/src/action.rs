//! Normal-mode key bindings shared by dispatch and the help overlay.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Semantic action to perform in normal mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Select the next visible branch.
    Down,
    /// Select the previous visible branch.
    Up,
    /// Select the first visible branch.
    Top,
    /// Select the last visible branch.
    Bottom,
    /// Scroll the detail pane one line down.
    DetailDown,
    /// Scroll the detail pane one line up.
    DetailUp,
    /// Scroll the detail pane one page down.
    DetailPageDown,
    /// Scroll the detail pane one page up.
    DetailPageUp,
    /// Open a PR in the browser.
    OpenPr,
    /// Checkout the selected branch.
    Checkout,
    /// Push the active stack.
    Push,
    /// Sync the active stack.
    Sync,
    /// Toggle PR draft state.
    ToggleDraft,
    /// Immediately refresh all sources.
    Refresh,
    /// Begin branch-name filtering.
    Filter,
    /// Cycle the row sort order.
    Sort,
    /// Show or hide bot reviewers.
    ToggleBots,
    /// Show keybinding help.
    Help,
    /// Exit the program.
    Quit,
}

/// Comparable key specification for a keymap entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySpec {
    /// Terminal key code.
    pub code: KeyCode,
    /// Required modifiers.
    pub modifiers: KeyModifiers,
}

/// One normal-mode binding and its in-app help text.
#[derive(Clone, Copy, Debug)]
pub struct KeyBinding {
    /// Which key triggers it.
    pub key: KeySpec,
    /// Which action is dispatched.
    pub action: Action,
    /// Short help caption.
    pub help: &'static str,
    /// Group in the help overlay.
    pub context: &'static str,
}

macro_rules! binding {
    ($key:expr, $modifiers:expr, $action:expr, $help:expr, $context:expr) => {
        KeyBinding {
            key: KeySpec {
                code: $key,
                modifiers: $modifiers,
            },
            action: $action,
            help: $help,
            context: $context,
        }
    };
}

/// All normal-mode keys, also used to generate help and footer hints.
pub const KEYMAP: &[KeyBinding] = &[
    binding!(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
        Action::Down,
        "next branch",
        "Table"
    ),
    binding!(
        KeyCode::Down,
        KeyModifiers::NONE,
        Action::Down,
        "next branch",
        "Table"
    ),
    binding!(
        KeyCode::Char('k'),
        KeyModifiers::NONE,
        Action::Up,
        "previous branch",
        "Table"
    ),
    binding!(
        KeyCode::Up,
        KeyModifiers::NONE,
        Action::Up,
        "previous branch",
        "Table"
    ),
    binding!(
        KeyCode::Char('g'),
        KeyModifiers::NONE,
        Action::Top,
        "first branch",
        "Table"
    ),
    binding!(
        KeyCode::Char('G'),
        KeyModifiers::SHIFT,
        Action::Bottom,
        "last branch",
        "Table"
    ),
    binding!(
        KeyCode::Char('o'),
        KeyModifiers::NONE,
        Action::OpenPr,
        "open PR",
        "Table"
    ),
    binding!(
        KeyCode::Char('c'),
        KeyModifiers::NONE,
        Action::Checkout,
        "checkout branch",
        "Table"
    ),
    binding!(
        KeyCode::Char('p'),
        KeyModifiers::NONE,
        Action::Push,
        "push stack",
        "Table"
    ),
    binding!(
        KeyCode::Char('S'),
        KeyModifiers::SHIFT,
        Action::Sync,
        "sync stack",
        "Table"
    ),
    binding!(
        KeyCode::Char('m'),
        KeyModifiers::NONE,
        Action::ToggleDraft,
        "toggle draft",
        "Table"
    ),
    binding!(
        KeyCode::Char('J'),
        KeyModifiers::SHIFT,
        Action::DetailDown,
        "scroll details down",
        "Detail"
    ),
    binding!(
        KeyCode::Char('K'),
        KeyModifiers::SHIFT,
        Action::DetailUp,
        "scroll details up",
        "Detail"
    ),
    binding!(
        KeyCode::PageDown,
        KeyModifiers::NONE,
        Action::DetailPageDown,
        "details next page",
        "Detail"
    ),
    binding!(
        KeyCode::PageUp,
        KeyModifiers::NONE,
        Action::DetailPageUp,
        "details previous page",
        "Detail"
    ),
    binding!(
        KeyCode::Char('R'),
        KeyModifiers::SHIFT,
        Action::Refresh,
        "refresh",
        "Global"
    ),
    binding!(
        KeyCode::Char('/'),
        KeyModifiers::NONE,
        Action::Filter,
        "filter branches",
        "Global"
    ),
    binding!(
        KeyCode::Char('s'),
        KeyModifiers::NONE,
        Action::Sort,
        "cycle sort",
        "Global"
    ),
    binding!(
        KeyCode::Char('b'),
        KeyModifiers::NONE,
        Action::ToggleBots,
        "toggle bots",
        "Global"
    ),
    binding!(
        KeyCode::Char('?'),
        KeyModifiers::SHIFT,
        Action::Help,
        "help",
        "Global"
    ),
    binding!(
        KeyCode::Char('q'),
        KeyModifiers::NONE,
        Action::Quit,
        "quit",
        "Global"
    ),
    binding!(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
        Action::Quit,
        "quit",
        "Global"
    ),
];

/// Find the normal-mode action for a press or repeat event.
#[must_use]
pub fn lookup(key: KeyEvent) -> Option<Action> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    KEYMAP
        .iter()
        .find(|binding| {
            binding.key.code == key.code
                && (binding.key.modifiers == key.modifiers
                    || (binding.key.modifiers == KeyModifiers::SHIFT
                        && key.modifiers == KeyModifiers::NONE
                        && matches!(key.code, KeyCode::Char('G' | 'S' | 'R' | '?' | 'J' | 'K'))))
        })
        .map(|binding| binding.action)
}
