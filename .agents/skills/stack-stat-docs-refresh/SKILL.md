---
name: stack-stat-docs-refresh
description: Use for a repository-wide documentation audit, refresh, or sweep, including stale links and shipped plans.
---

# Refresh living docs

Read [documentation conventions](../../../docs/README.md). A change is complete only when statements are accurate, links resolve, and commands correspond to the actual workspace.

1. Check [plans](../../../docs/plans/) for shipped work. Invoke `stack-stat-docs-absorb-plan` for completed plans; leave in-flight plans alone.
2. Resolve every local Markdown link in root README/AGENTS and living docs; fix dead links instead of merely reporting them.
3. Compare architecture claims, crate names, command examples, and Mermaid nodes with code. Fix inaccuracies; flag a section that cannot be corrected without a design change.
4. Remove rot-prone counts, duplicated signatures, exhaustive inventories, and filler; retain meaningful rationale.
5. Ensure the architecture docs cover the actual providers, data model, and TUI; add an ADR via `stack-stat-docs-create-adr` only for a decision whose alternatives matter or which is costly to reverse.
6. Build rustdoc (`cargo doc --workspace --no-deps --locked`) if the tooling is available. Report exact checks and changes; commit the docs fixes by concern.

If a class of inaccuracy recurs, propose a targeted update to this skill rather than silently expanding its rules.
