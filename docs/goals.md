# Goals

Judge architecture choices against these goals. Name any tradeoff that improves one at another's expense.

## At-a-glance stack health

A developer can understand branch, pull request, check, and review state without leaving the terminal.

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
