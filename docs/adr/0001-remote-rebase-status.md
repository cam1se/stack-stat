# ADR-0001: Derive rebase status from remote state

- **Status:** Proposed
- **Date:** 2026-10-13
- **Deciders:** tcamise

## Context

The dashboard answers one question: can each layer of the stack merge? Checks, reviews, threads, and draft state all come from GitHub. Rebase status came from a different source, the `needsRebase` field of `gh stack view --json`. That field is true when the parent's current tip is not an ancestor of the *local* branch, so it reflects the checkout and the last `git fetch`, not GitHub.

In a real stack (`gopro/infra-katalyst-platform`, PRs #1827 and eight layers above it), `gh stack` flagged every layer above the bottom open PR, so the dashboard marked them all as needing a rebase. GitHub's compare API reported `behind_by: 0` for every layer against its base. The verdict was wrong for anyone whose local branches were stale, and it blocked readiness for layers that were mergeable. A branch-hopping agent makes stale local state the common case.

Forces:

- Readiness must come from the same source as the other columns, so one row does not mix remote and local facts.
- Merge and CI act on the remote branch; the remote is what reviewers see.
- Local drift is still useful, but as information, not as a merge gate.
- The poll must not grow by one request per layer.

## Decision

A non-merged layer needs a rebase only when GitHub shows that its parent ref has commits absent from its head.

- **Parent:** the first non-merged layer compares against trunk. Each later non-merged layer compares against the nearest non-merged layer below it. Merged layers are skipped.
- **Data:** the existing aliased GraphQL poll gains one comparison per layer, `repository.ref(qualifiedName: <parent>) { compare(headRef: <layer>) { behindBy } }`. A positive `behindBy` means the layer needs a rebase.
- **Failure:** a missing ref, failed comparison, or null result means no rebase need. The dashboard never reports a rebase it cannot show evidence for.
- **Cascade:** layers above a layer that needs a rebase are marked as moving with it, with the same `Stacked` semantics as before.
- **Removed input:** `StackBranch.needs_rebase` is deleted. `gh stack view --json` still supplies layer order, PR identity, and merged/queued state.
- **Local drift:** the worktree's branch is compared with its remote-tracking ref and shown as a muted gutter marker (`*↑`, `*↓`, `*⇅`, `*?`) and in the details pane. It never affects the verdict.

## Consequences

- The rebase column agrees with checks, reviews, and threads, and no longer depends on the freshness of a local fetch.
- A stale checkout no longer blocks readiness; it shows up as local drift on the current row only.
- The poll stays one GraphQL request. Its size grows linearly with the number of layers.
- A rebase done locally but not yet pushed does not clear the rebase flag until the push; the `*↑` marker shows the unpushed commits.
- The local marker is only as fresh as the last fetch, because it uses the remote-tracking ref rather than a live remote read.
- Fail-open on missing comparisons can hide a real rebase when GitHub returns no comparison. This was chosen over a false alarm that blocks a mergeable layer.
- The dashboard depends on GitHub's `compare` semantics for parent refs. Deleted parent branches produce no comparison and so no rebase status.

## Alternatives considered

- **Keep `gh stack`'s `needsRebase`.** Rejected: it is local-state-derived and produced the false positives that motivated this change.
- **Show both local and remote rebase as separate columns.** Rejected: two rebase signals in the same table are hard to read, and only the remote one answers the readiness question. Local state is available as the current-row marker instead.
- **Compare with the REST `compare` endpoint per layer.** Rejected: one request per layer per poll adds latency and rate-limit cost. The GraphQL comparison reuses the existing request.
- **Compare each PR's `baseRefOid` with the parent's `headRefOid`.** Rejected: it compares two tips, not ancestry, so it can flag a layer that already contains the parent's commits through a rebase or merge.
- **Fetch before every poll to refresh the local view.** Rejected: it mutates the user's repository from a read-only dashboard and still tests the local branch, not the remote one.

## Related documentation

- [Domain and configuration](../architecture/04-domain-and-config.md): readiness rules and rebase semantics.
- [Providers and store](../architecture/05-providers-and-store.md): the GraphQL comparison and its failure handling.
- [TUI](../architecture/06-tui.md): the local-sync marker.
