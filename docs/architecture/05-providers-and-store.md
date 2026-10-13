# 05 — Providers and local status store

The [core library](../../crates/stack-stat-core/) owns the `GitProvider`, `GitHubProvider`, and `StatusStoreProvider` contracts. The TUI consumes those interfaces; production adapters run external processes or read files, while tests inject fakes. Provider types retain upstream JSON vocabulary. The [domain model](04-domain-and-config.md) has no CLI or GraphQL dependency.

The trait boundary makes branch switches, check transitions, and error paths testable with mutable in-memory fakes without live GitHub access. Fake executables on PATH would couple App behavior to CLI flag order and platform-specific scripts; a replay-only mode could not drive transitions or partial failures. The extra trait implementations are the deliberate cost of a stable, testable boundary.

## Git adapter

`CliGitProvider` discovers the checkout's top-level path and the Git common directory, which is shared by all worktrees. It reads HEAD via `git symbolic-ref --short HEAD`, branch refs via a single `git for-each-ref` invocation, and resolves explicit revisions with `git rev-parse --verify`. A detached checkout is represented by a distinct marker. Subprocess stdio is piped and optional Git locks are disabled for polling so the viewer does not interfere with concurrent agent work.

## GitHub adapter

`CliGitHubProvider` resolves the repository owner and name once through `gh repo view`, reads the checked-out branch's stack with `gh stack view --json`, then fetches PR details by exact PR number in one aliased `gh api graphql` query. PR details include draft state, the latest reviews and outstanding review requests, and head-commit check runs plus legacy status contexts. A GraphQL response with partial repository data is parsed even if `gh api` exits nonzero because one alias failed; deleted or inaccessible PR aliases are omitted while valid siblings remain available. A local stack can omit `head` and `pr` until a layer has been submitted; Git's branch refs supply the current tip.

Every `gh` process runs with piped input/output and prompts disabled. Read operations time out after 20 seconds; push and sync allow 120 seconds. The adapter classifies off-stack, rebase-conflict, rate-limit, authentication, network, and parse failures separately so the UI can retain the last useful snapshot or direct the user to an actionable fix. `gh stack sync` can exit 0 while printing `Sync aborted` when local and remote stacks diverge; the adapter reads both stdout and stderr and reports that as an abort, never as a successful sync. The adapter does not decide colors, sorting, or which checks the repository suppresses.

## Local check results

The file store lives at `<git-common-dir>/stack-stat/status/<encoded-branch>/<full-sha>.json`. Branch names are encoded as one platform-safe directory segment; `feat/api` does not create nested directories. The file contains a map of named results with outcome, optional message, and UTC timestamp. Cooperative readers take a shared lock and reporters/cleanup take an exclusive lock on a separate `status.lock` file. Writing merges a named result with its existing file and atomically replaces the file from a temporary sibling. This prevents a Windows reader's open handle from blocking replacement, while preserving complete JSON for readers across processes and worktrees. If no store exists, reads return empty without creating a lock file.

Malformed files are skipped with a warning while other entries remain visible. A new report may replace a malformed file. Cleanup retains each live branch's current-SHA entry and the most recently recorded older entry, deleting earlier entries and deleted branch directories. Retaining the older file is necessary to display a stale result after a commit or rebase. Both the file store and the in-memory fake use the same pure retention rule.

The Git common directory scopes reports to the clone and shares them among its worktrees. A user data directory would need a separate identity and cleanup policy across renamed and duplicate clones; tracked report files would turn transient, commit-specific evidence into source history.

## Adding provider behavior

Add a method to the narrow trait in [the providers module](../../crates/stack-stat-core/src/providers/mod.rs), implement the CLI/file adapter and fake with the same semantics, and add an observable contract test in the owning crate. Keep transport error classification at the adapter boundary; keep presentation decisions in the compositor.
