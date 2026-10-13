# 05 — Providers and local status store

The [core library](../../crates/stack-stat-core/) owns the `GitProvider`, `GitHubProvider`, and `StatusStoreProvider` contracts. The TUI consumes those interfaces; production adapters run external processes or read files, while tests inject fakes. Provider types retain upstream JSON vocabulary. The [domain model](04-domain-and-config.md) has no CLI or GraphQL dependency.

## Git adapter

`CliGitProvider` discovers the checkout's top-level path and the Git common directory, which is shared by all worktrees. It reads HEAD via `git symbolic-ref --short HEAD`, branch refs via a single `git for-each-ref` invocation, and resolves explicit revisions with `git rev-parse --verify`. A detached checkout is represented by a distinct marker. Subprocess stdio is piped and optional Git locks are disabled for polling so the viewer does not interfere with concurrent agent work.

## Local check results

The file store lives at `<git-common-dir>/stack-stat/status/<encoded-branch>/<full-sha>.json`. Branch names are encoded as one platform-safe directory segment; `feat/api` does not create nested directories. The file contains a map of named results with outcome, optional message, and UTC timestamp. Writing merges a named result with its existing file and atomically replaces the file from a temporary sibling, so readers never observe a partial JSON document.

Malformed files are skipped with a warning while other entries remain visible. A new report may replace a malformed file. Cleanup retains each live branch's current-SHA entry and the most recently recorded older entry, deleting earlier entries and deleted branch directories. Retaining the older file is necessary to display a stale result after a commit or rebase. Both the file store and the in-memory fake use the same pure retention rule.

## Adding provider behavior

Add a method to the narrow trait in [the providers module](../../crates/stack-stat-core/src/providers/mod.rs), implement the CLI/file adapter and fake with the same semantics, and add an observable contract test in the owning crate. Keep transport error classification at the adapter boundary; keep presentation decisions in the compositor.
