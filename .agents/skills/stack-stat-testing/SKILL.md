---
name: stack-stat-testing
description: Use when deciding tests for stack-stat features, bug fixes, App transitions, provider behavior, CLI contracts, or terminal rendering.
---

# Choose valuable tests

Read [the testing discipline](../../../AGENTS.md) first. A bug fix needs a regression test that fails before the fix; a new contract needs coverage for behavior, boundaries, and plausible errors. Do not test forwarding, source text, exact incidental wording, field copies, or duplicate paths.

| Contract | Location | Approach |
|---|---|---|
| Config, model, compositor | `crates/stack-stat-core/tests/` | Pure domain tests |
| Git and status store | `crates/stack-stat-core/tests/` | Isolated temp git repo and temp store |
| App, polling, actions | `crates/stack-stat-tui/tests/` | Fake providers, drive `App` directly without sleeping |
| Rendering | `crates/stack-stat-tui/tests/` | `ratatui::TestBackend` buffer assertions |
| CLI subcommands | `crates/stack-stat-tui/tests/` | Invoke binary in a temp repo; no real `gh` or network |

Use the narrow read-only App accessors and injected timestamps. A provider fake models a boundary, not an expected echo from a mock. Test user-visible state after a transition. PTY and visual snapshots are out of scope: direct App behavior and frame buffer checks defend the contracts more reliably.

Rewrite brittle tests rather than pinning implementation details. If a test appears valueless, seek user approval before deleting it. Run a focused file with `cargo test -p <package> --test <file>` while iterating; finish with `cargo test --workspace --locked --no-fail-fast` and the lint commands in AGENTS.md.
