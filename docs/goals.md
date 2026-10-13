# Goals

Judge architecture choices against these goals. Name any tradeoff that improves one at another's expense.

## Know at a glance whether a branch is ready

The primary purpose. A developer sees from the stack table alone whether each branch is ready to merge, without leaving the terminal. Readiness combines draft status, CI, reviews, and local checks, and is never overstated: unknown or failing evidence is not shown as ready.

## Find the blocker fast

When a branch is not ready, the blocking check, review, rebase, or missing result is reachable within a keystroke or two of the row. The dashboard shows what is wrong, not just that something is.

## Agent-aware

The checked-out worktree branch stays visible as an agent changes branches. Status is never confused with the selected row.

## Actionable

Stack operations are accessible without abandoning the dashboard, with an explicit confirmation before mutations.

## Local checks

Agents, scripts, and hooks can report reproducible results for the commit they tested. The UI distinguishes fresh, stale, and missing results.

## Per-repository relevance

Repositories can declare expected checks and hide noisy CI without changing the underlying reported state.

## Self-updating

The released binary installs and updates with verifiable checksums; users remain in control of when it changes.

## Built for AI-agent development

Focused modules, maintained docs, deterministic tests, and explicit contracts make changes independently understandable and verifiable.
