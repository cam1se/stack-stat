# ADR-0002: Read stack structure from GitHub; use gh stack only for actions

- **Status:** Accepted
- **Date:** 2026-10-13
- **Deciders:** tcamise

## Context

[Issue #16](https://github.com/cam1se/stack-stat/issues/16) exposed a mismatch between GitHub and local stack tracking: PRs [#13](https://github.com/cam1se/stack-stat/pull/13) and [#14](https://github.com/cam1se/stack-stat/pull/14) belonged to GitHub stack #15, but a worktree without local tracking reported that it was not in a stack. A dashboard must work in any clone or worktree with access to those PRs, without requiring a mutating setup command.

[ADR-0001](0001-remote-rebase-status.md) already moved rebase readiness to GitHub. Stack membership and layer order were the remaining read inputs from `gh stack view --json`. Its assumption that the extension is a hard prerequisite for reading is narrowed by this decision; the remote rebase rule and informational local drift remain unchanged.

### Spike findings

A read-only GraphQL spike against `cam1se/stack-stat` confirmed the following:

1. **Stack API:** `PullRequest.stack` exposes `PullRequestStack` with `id`, `number`, `size`, `baseRefName`, and ordered entries. Each `PullRequestStackEntry` has a `position` and its `pullRequest`. Entry PRs expose their number, URL, head/base ref names, lifecycle state, and `isInMergeQueue`. An unstacked PR returns `stack: null`. There is no need to reconstruct membership from PR bases or rely on a REST chain walk.
2. **Base-chain counterexample:** both merged PRs #13 and #14 still belong to stack #15, but both report `baseRefName: main`. Walking bases would treat them as siblings and lose the real layer order. Entry positions preserve that order.

The branch lookup can select `repository.pullRequests(headRefName: <local branch>, states: [OPEN, MERGED, CLOSED])` and request each PR's `stack`. Looking up a known PR number through `repository.pullRequest(number: <number>)` exposes the same stack field. Including merged and closed PRs matters for completed stacks.

Other base-chain edge cases were not separate live spike tests, but reinforce why base refs are not a membership protocol:

- A branch with no PR supplies no GitHub stack identity.
- A deleted merged parent ref cannot support a branch walk; PR and stack metadata, rather than live local refs, supply membership.
- Retargeting changes a PR's current base, not a reliable history of stack order.
- Forks can reuse branch names, so a ref name alone is not a PR identity; discovery remains scoped to the resolved GitHub repository.
- PRs sharing a base are not necessarily a stack. A non-null GitHub stack and its entry positions, not a common base, establish membership and order.

## Decision

Use GitHub as the single read authority for stack membership, order, and PR lifecycle. Remove the local stack-view read; do not retain it as a fallback. Local Git remains the authority for the checked-out branch and local commit state.

### Field mapping

| Previous stack-view input | Authority after cutover |
|---|---|
| Stack identity and trunk | `PullRequest.stack` identity and `baseRefName` |
| Layer order | Stack entry `position`, from trunk to top |
| Branch name and PR identity | Entry PR `headRefName`, `number`, and `url` |
| `isQueued` | PR `isInMergeQueue` |
| `isMerged` | PR `state == MERGED` |
| Current branch / `isCurrent` | Checked-out branch from local Git; match it to the layer branch |
| `head` / `base` | Local Git refs supply local head SHAs for commit-specific checks; PR head/base refs and the stack trunk supply remote ref names. Extension-recorded base/head OIDs do not determine membership or readiness. |

Remote rebase checks continue to compare the first non-merged layer against trunk and subsequent non-merged layers against their nearest non-merged predecessor. A missing or failed comparison still means no demonstrated rebase need, as in ADR-0001.

### Actions and local tracking

Keep `gh stack push`, `gh stack sync`, and `gh stack checkout` for manipulation. Push and sync need local tracking; a successful remote read does not imply that tracking exists.

When a confirmed push or sync reports missing local tracking, prompt before linking. On approval, run `gh stack checkout <pr-number-or-branch>` using the current worktree branch's PR number, or its branch name when no number is available, then run the original action. Checkout is the linking operation; do not invent a separate `gh stack link` dependency. Declining is a no-op with a status message: no checkout, linking, or action retry. Reads never initialize, link, fetch, or reconcile a local stack.

The CLI adapter's [error classifier](../../crates/stack-stat-core/src/providers/graphql.rs) treats push/sync exit code 2 as missing local tracking (`LocalTrackingMissing`), distinct from remote `NotInStack`, and stack exit codes 3 or 7 as rebase conflicts. Missing local tracking on an action is not evidence that the remote stack disappeared. Do not automatically link or retry a conflict. An exit-0 sync printing `Sync aborted` remains an abort, not success.

### Prerequisites and missing data

Startup requires `gh` and authentication, not the `gh stack` extension. Check extension availability on checkout, push, and sync and report the actionable installation command, `gh extension install github/gh-stack`, when it is absent. PR reads, draft/ready changes, and opening a PR do not require the extension.

The missing-stack equivalent of ADR-0001's fail-open rule is the existing **Not in a stack** view, not a fabricated stack or a fallback to local tracking. A branch with no PR, or a PR with `stack: null`, clears previous rows. Tell the user to open and stack PRs on GitHub or switch to a stacked PR branch, not to initialize a local stack. Authentication, rate-limit, network, and malformed-response failures remain fetch errors that retain the last useful snapshot; they are not silently converted to absence.

### Poll cost

Stack discovery must precede the existing aliased detail/comparison query: entry PR numbers and the ordered remote parent refs are needed to construct its aliases. GraphQL sibling selections cannot use the result of discovery as another selection's arguments. Use one stack-discovery request, then one aliased request for all PR details and comparisons: **one additional GraphQL round trip per ordinary in-stack poll**, not one request per layer. Stacks with more than 100 entries need additional discovery pages. An off-stack lookup needs only discovery.

## Consequences

- Existing GitHub stacks are visible from fresh clones and worktrees without local stack setup.
- Membership, lifecycle, and readiness come from GitHub; local drift and check freshness remain local information.
- Branches without PRs are not remote stack layers. The dashboard does not expose unpublished local-only layers through a second read path.
- Deleted or retargeted refs cannot justify inventing an alternative order from base names. GitHub's stack metadata is the authority.
- Linking remains an explicit mutation. Approval may change the worktree through checkout, so the prompt must identify the target before continuing push or sync.
- The extension remains an action dependency, and discovery adds a bounded request cost. API access failures remain visible while preserving the last useful data.

## Alternatives considered

- **Reconstruct from PR base chains.** Rejected: the verified merged-stack counterexample loses order, and shared, retargeted, deleted, or forked refs do not establish stack membership.
- **Keep `gh stack view` as a fallback.** Rejected: it restores the clone-dependent behavior and introduces two authorities with different failure and freshness semantics.
- **Require local checkout/linking before showing a stack.** Rejected: reading should not mutate repository state or require the action extension.
- **Automatically link before push or sync without asking.** Rejected: checkout and tracking changes are consequential mutations. The user must approve the target.
- **Combine discovery and dynamic comparison aliases in one initial request.** Rejected: alias arguments depend on stack entries returned by discovery. A separate bounded discovery request preserves the existing batched detail and comparison behavior.

## Related documentation

- [Providers and store](../architecture/05-providers-and-store.md): read transport, normalization, and action failures.
- [System context](../architecture/01-l1-context.md): GitHub authority and extension scope.
- [TUI](../architecture/06-tui.md): preflight, confirmation, and not-in-stack behavior.
- [Troubleshooting](../user/troubleshooting.md): missing PRs, extension installation, and local tracking.
