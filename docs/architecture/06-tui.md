# 06 — Terminal application

The default binary command checks for authenticated `gh` and the `gh stack` extension before entering raw terminal mode. The [core providers](05-providers-and-store.md) supply Git, GitHub, and local check snapshots. The App owns the most recent successful slice of each source; its pure [compositor](04-domain-and-config.md) creates the view used by rendering.

```mermaid
flowchart LR
  subgraph Background
    Git[Git poller]
    Remote[GitHub poller]
    Local[Status store poller]
  end
  subgraph Foreground
    Messages[mpsc messages]
    App[App and input modes]
    Frame[ratatui frame]
  end
  Git --> Messages
  Remote --> Messages
  Local --> Messages
  Messages --> App --> Frame
  App -->|effects| Remote
```

## Messages and input modes

The main loop selects terminal input, provider messages, and a presentation tick. Provider errors retain the last successful snapshot, except an explicit `NotInStack` result, which clears rows as the worktree leaves its stack. A change of the checked-out branch updates the worktree marker on the next Git poll. The App exposes narrowly scoped input, snapshot, effect, and tick methods that tests can drive with fake providers and an injected timestamp; no test needs a terminal or wall-clock sleep.

The keymap is one table shared by input dispatch and the help overlay. Normal mode handles navigation and actions. Filter mode edits a branch-name query. Help consumes its next key without navigating. Mutations use a confirmation mode that accepts only `y`; another key cancels. Only one mutation can be in flight. Effects run outside the key handler, then request an immediate refresh on completion. Terminal raw mode, alternate screen, and cursor state are restored on normal exit, errors, and panic.

## Frame layout and theme

The header reserves separate lines for stack order and the worktree branch so the branch remains visible when a chain is too wide. The body is a compact status table. Its columns show branch and PR identity, draft/ready/merged/queued lifecycle, a filtered CI summary, visible review state, and reported local checks. The checked-out branch and selected row are distinct: `←wt` marks the former and `►` marks the latter. Enter or Tab opens a selected-branch detail pane with every check, reviewer, local result, and rebase warning. `j/k` and the mouse wheel over the table move the selected branch; `J/K` and Page Up/Down scroll detail lines without changing selection. The mouse wheel scrolls whichever pane it hovers, and clicking a table row selects it. No focus-navigation mode is needed. Overflowing details show a continuation marker until scrolled to the end. The footer provides context-sensitive key hints, confirmations, fetch errors, and update notices. Help is generated from the same keymap used for dispatch and uses two columns on shorter terminals.

At widths below 100 columns, reviews collapse to an icon; below 80, local checks collapse to a worst-state icon. Branch names truncate before fixed status columns shrink. The renderer reads only App state and its injected clock value. Theme selection happens once on launch: `COLORTERM` selects truecolor when available, `TERM` falls back to indexed colors, and basic ANSI colors remain usable otherwise. Animation applies to the active branch, running checks, changed cells, loading placeholders, and activity/freshness indicators as described in the [design](../specs/001-stack-stat-design.md).

## Refresh policy

Git and the local store poll independently at their configured intervals. GitHub polls more often while checks are running or queued, and pauses after a rate-limit error until a parseable reset or the fallback delay. A forced refresh wakes pollers without waiting for their normal timers. Store cleanup waits for an initial Git branch map so it does not delete reports before learning which branches exist.

## Extending a keybinding

Add the semantic action and one entry to [the keymap](../../crates/stack-stat-tui/src/action.rs), handle it in the appropriate App input mode, and cover its observable state/effect in [behavioral tests](../../crates/stack-stat-tui/tests/app_state.rs). Help uses the same keymap; it does not need a second manually maintained list. For layout changes, add [TestBackend](../../crates/stack-stat-tui/tests/) assertions when rendering ships.
