# ADR-0003: Show a single PR or current branch without a GitHub stack

- **Status:** Accepted
- **Date:** 2026-10-13
- **Deciders:** tcamise
- **Supersedes:** [ADR-0002](0002-read-stack-from-github.md), only its missing-data behavior and action requirements for branches without a GitHub stack. GitHub stack discovery, ordering, and stacked actions remain unchanged.

## Context

[Issue #26](https://github.com/cam1se/stack-stat/issues/26) requires the dashboard to remain useful for ordinary, non-stacked development. ADR-0002 treats both a PR with `stack: null` and a branch with no matching PR as **Not in a stack**, hiding PR details and commit-specific local checks that are still meaningful. Displaying these branches must not invent GitHub stack membership or a merge-ready PR.

## Decision

Keep GitHub as the authority for real stack membership. Distinguish stacked mode from single mode by the presence of GitHub stack metadata, not the number of rows: a non-null stack with one entry remains stacked mode.

- A selected PR with `stack: null` produces one single-mode entry. Its base branch is trunk. Reuse the existing PR detail query for lifecycle, draft state, checks, reviews, and remote rebase comparison against that trunk.
- An attached current branch with no matching PR produces one single-mode row. The repository's GitHub default branch is trunk. Show a `no PR yet` hint, leave PR-only cells empty, and retain local sync information and commit-specific local checks, including failed, stale, and missing results. This row is not a ready-to-merge PR and is excluded from `Next to merge` selection. The default branch itself has nothing to compare against, so without a PR it produces `NotInStack`, labelled as the default branch rather than as a detached HEAD.
- Detached or empty HEAD may still produce `NotInStack`. Authentication, network, rate-limit, and malformed-response failures remain errors, not proof that no PR exists.
- A selected PR closed without merging is closed history: it has no readiness issues, is never ready to merge or the next PR to merge, and cannot toggle draft state.

Single-mode push runs plain `git push` after confirmation. Preserve ordinary Git upstream/configuration errors; do not initialize or link a stack. Single-mode sync and checkout are disabled with a clear status and no extension invocation. Opening a PR and toggling draft/ready state remain available when a PR exists; a no-PR row has no PR to act on.

Startup still requires authenticated `gh`, but no stack extension. Only stacked checkout, push, and sync require `gh stack` and retain ADR-0002's explicit local-tracking confirmation behavior. Discovery of a non-null GitHub stack continues to use its trunk and entry positions without inferring membership from PR bases.

## Consequences

- Ordinary PRs get the same readiness evidence as stacked PRs, while branches before PR creation still expose local-check evidence.
- Mode must remain explicit through composition and action dispatch; row count cannot determine which push command or actions to use.
- A successful no-PR discovery replaces earlier rows without turning a local branch into a merge candidate. Refresh failures still preserve the last useful snapshot.
- A single-mode push without a configured upstream can fail normally and requires the user to configure Git outside the dashboard.

## Alternatives considered

- **Keep the not-in-stack view for all absent stack metadata.** Rejected: it discards useful PR and local-check evidence during ordinary development.
- **Treat every one-row view as a stack and use `gh stack` actions.** Rejected: an unstacked PR has no stack membership, and a genuine one-entry GitHub stack must retain stacked behavior.
- **Reconstruct a stack from PR bases or local tracking.** Rejected for the authority and ordering reasons in ADR-0002; a single-mode row is not inferred stack membership.

## Related documentation

- [Providers and store](../architecture/05-providers-and-store.md): discovery normalization, trunk selection, and action adapters.
- [Terminal application](../architecture/06-tui.md): mode-aware actions and no-PR presentation.
- [System context](../architecture/01-l1-context.md): read authority and extension scope.
- [User guide](../user/index.md): single-mode usage and `Next to merge`.
- [Troubleshooting](../user/troubleshooting.md): no PR, disabled actions, and upstream errors.
