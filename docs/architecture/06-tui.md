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

The main loop selects terminal input, provider messages, and a presentation tick. Provider errors retain the last successful snapshot, except an explicit `NotInStack` result, which clears rows as the worktree leaves its stack. Fetch errors are tracked per source, so a successful local poll does not erase an outstanding GitHub warning; failed one-off mutations are transient and never suppress later fetch errors. A confirmation prompt remains visible during unrelated polls, and opening a PR does not release the busy guard on a concurrent push. A change of the checked-out branch updates the worktree marker on the next Git poll. The App exposes narrow input, snapshot, effect, and tick methods that tests can drive with fake providers and an injected timestamp; no test needs a terminal or wall-clock sleep.

The keymap is one table shared by input dispatch and the help overlay. Normal mode handles navigation and actions. Filter mode edits a branch-name query. Help consumes its next key without navigating. Mutations use a confirmation mode that accepts only `y`; another key cancels. Only one mutation can be in flight. Effects run outside the key handler, then request an immediate refresh on completion. Terminal raw mode, alternate screen, and cursor state are restored on normal exit, errors, and panic.

## Frame layout and theme

At widths below 100 columns, reviews collapse to an icon; below 80, local checks collapse to a worst-state icon. Branch names truncate before fixed status columns shrink. Status glyphs are single-width Unicode symbols, never emoji, because emoji widths vary by terminal and misalign columns. The renderer reads only App state and its injected clock value. Pane borders use a calm shade of the logo blue, pane titles the logo blue, and keys in the footer plus identifying values in the header (trunk, worktree branch, PR numbers) use the logo cyan; status colors stay reserved for state. Theme selection happens once on launch: `COLORTERM` selects truecolor when available, `TERM` falls back to indexed colors, and basic ANSI colors remain usable otherwise.


## Animation model

The App owns a monotonic presentation tick advanced every 100 ms. A 20-tick sine wave pulses the selected row; basic 16-color terminals pulse by toggling bold instead of changing the background. Queued and running checks use a braille spinner in both table and detail. When a successful provider update changes visible check, review, state, branch, PR, or local-result cells, those cells flash for five ticks. Unchanged polls do not retrigger flashes. Loading placeholders shimmer by column, the header spins while a fetch is in flight, and stale GitHub age pulses after twice the active polling interval. Rendering computes these effects from App state and the injected time; it never reads the wall clock itself.

## Refresh policy

Git and the local store poll independently at their configured intervals. GitHub polls more often while checks are running or queued, and pauses after a rate-limit error until a parseable reset or the fallback delay. A forced refresh wakes pollers without waiting for their normal timers. Store cleanup waits for an initial Git branch map so it does not delete reports before learning which branches exist.

## Extending a keybinding

Add the semantic action and one entry to [the keymap](../../crates/stack-stat-tui/src/action.rs), handle it in the appropriate App input mode, and cover its observable state/effect in [behavioral tests](../../crates/stack-stat-tui/tests/app_state.rs). For layout changes, add [TestBackend assertions](../../crates/stack-stat-tui/tests/render.rs) when rendering ships.
