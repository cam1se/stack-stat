---
name: stack-stat-commit
description: Use when asked to commit or save changes in stack-stat. Guide a focused Conventional Commit using the repository template.
---

# Commit a change

Read [`.gitmessage`](../../../.gitmessage) for authoritative types and scopes; do not trust a cached list. Inspect `git status --short` and `git diff --cached --stat` before staging or committing. Never stage unrelated user changes.

Infer a narrow scope from the touched path:

| Path | Scope |
|---|---|
| `crates/stack-stat-core/src/providers/file_store*` | `store` |
| `crates/stack-stat-core/src/providers/` | `providers` |
| `crates/stack-stat-core/src/config*` | `config` |
| Other `crates/stack-stat-core/` | `core` |
| `crates/stack-stat-tui/src/report*` | `report` |
| `crates/stack-stat-tui/src/update*` | `update` |
| Other `crates/stack-stat-tui/` | `tui` |
| `.github/workflows/` | `ci` |
| `.agents/` | `ai-infra` |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | `build` |
| Documentation | `docs` type with no scope |

For changes spanning concerns, pick the dominant scope or omit it. Imperative lowercase subject, no final period; explain why in a body when non-obvious. Use `BREAKING CHANGE:` or `Closes: #N` footers where applicable. Run the relevant verification before committing. Stage only relevant files, then commit with `git commit -m '<type>(<scope>): <subject>'` (body via another `-m` when needed). Confirm with `git log -1 --oneline`.

If a needed type or scope is missing, use the closest valid value for this change, then propose an addition to `.gitmessage` and this skill; ask before changing either. PR preparation and history cleanup belong to `stack-stat-ship` and `stack-stat-reflow`.
