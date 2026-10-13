# Contributor and agent guide

Build, test, and commit stack-stat. Read the [documentation conventions](docs/README.md) before editing docs.

> **Cross-platform target:** The workspace builds and tests on macOS, Linux, and Windows. Do not assume Unix path separators, line endings, or signals.

## Workspace layout

| Crate | Role |
|---|---|
| [`stack-stat-core`](crates/stack-stat-core/Cargo.toml) | Domain model, configuration, status store, git/GitHub providers |
| [`stack-stat-tui`](crates/stack-stat-tui/Cargo.toml) | `stack-stat` binary, TUI, `report` and `update` commands |

Architecture starts with [system context](docs/architecture/01-l1-context.md).

## Prerequisites

- Stable Rust, selected by [`rust-toolchain.toml`](rust-toolchain.toml).
- Git, GitHub CLI (`gh`), and an authenticated account (`gh auth login`).
- The stacked PR extension: `gh extension install github/gh-stack`.

First-time setup: `make dev-setup` checks prerequisites and installs versioned git hooks.

## Build and run

```sh
cargo build
cargo run -- --version
cargo run
```

The dashboard requires the current directory to be inside a Git repository. An active stack supplies the layer rows; outside a stack, the dashboard shows a not-in-stack view. The version command does not require a repository.

## Test

```sh
cargo test --workspace --locked --no-fail-fast
cargo test -p stack-stat-core
cargo test -p stack-stat-tui --test render
```

For one crate use `cargo test -p <package>`. For one integration test file use `cargo test -p <package> --test <file>`.

## Format and lint

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Run both before a commit. The [pre-commit hook](.githooks/pre-commit) checks staged Rust changes; `make dev-setup` installs it. CI runs the same commands and the full test suite. Run `make preflight` for the CI-equivalent gate.

## Module size and structure

Avoid monolithic Rust files. Around 300 lines, look for a cohesive seam; when a file crosses that threshold, propose a split before making it. Organize by responsibility, keep a thin facade, and document public items with rustdoc. Test and generated files are exempt from the line signal, not from the single-purpose principle.

## Commit conventions

Use [Conventional Commits](https://www.conventionalcommits.org/) with types and scopes from [`.gitmessage`](.gitmessage). Use an imperative lowercase subject without a trailing period. Explain non-obvious changes in the body; use `BREAKING CHANGE:` and `Closes:` in the footer where applicable. Keep one concern per commit.

Agents should follow [commit guidance](.agents/skills/stack-stat-commit/SKILL.md) when preparing a commit; [reflow](.agents/skills/stack-stat-reflow/SKILL.md) and [ship](.agents/skills/stack-stat-ship/SKILL.md) cover PR preparation.

## Pull requests

Branch from `main`, keep each PR focused, and update documentation with code. Before pushing, run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked --no-fail-fast`. Investigate failed CI checks rather than ignoring them.

## Documentation discipline

Documentation moves with code. Structural changes update [architecture](docs/architecture/01-l1-context.md); hard-to-reverse decisions get an [ADR](docs/adr/README.md). The [docs conventions](docs/README.md) define one source of truth per fact, navigable links, and living versus point-in-time documents.

For a full documentation audit, use [docs refresh](.agents/skills/stack-stat-docs-refresh/SKILL.md); [ADR authoring](.agents/skills/stack-stat-docs-create-adr/SKILL.md) and [plan absorption](.agents/skills/stack-stat-docs-absorb-plan/SKILL.md) explain durable decisions.

## Notes for coding agents

Read [docs conventions](docs/README.md) before writing docs. Verify behavior with a command or test rather than an assertion. Run full lint/test once when a change is complete, not after every edit.

## Testing discipline

A bug fix or feature needs tests for its observable contract. A bug regression test must fail before the fix and pass afterward. Cover behavior, boundaries, and error paths for new APIs; do not add tests for mechanical wiring or duplicate coverage. Tests must be deterministic, isolated, and resilient to wording changes. Avoid wall-clock sleeps and shared mutable fixtures.

See the [testing skill](.agents/skills/stack-stat-testing/SKILL.md) for the layer-specific brittleness checklist.

Do not change production invariants or expose broad internals solely for tests. Prefer pure functions and narrow interfaces. Never delete a test without explicit user approval; investigate or rewrite a brittle test first.

| Layer | Location | Focus |
|---|---|---|
| Core contracts | `crates/stack-stat-core/tests/` | Config, providers, compositor, store |
| App state machine | `crates/stack-stat-tui/tests/` | Fake providers, state transitions, action effects |
| Rendering | `crates/stack-stat-tui/tests/` | `ratatui::TestBackend` frame contracts |
| CLI | `crates/stack-stat-tui/tests/` | Report and update behavior against isolated temp repos |

Do not add PTY-based integration tests or visual snapshots: direct App behavior and `TestBackend` buffer checks are more precise.
