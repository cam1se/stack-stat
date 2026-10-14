# ADR-0004: Validate gh and gh stack against a live fixture

- **Status:** Proposed
- **Date:** 2026-10-13
- **Deciders:** tcamise

## Context

Every test ran against fakes or fixtures, so assumptions about `gh` and `gh stack` output and exit codes went unchecked. Probing the real binaries while building this showed three of them wrong or imprecise:

- `gh` 2.101.0 installs a missing `stack` extension on first use and exits 0 without running the command, so the "unknown command" classifier never matched and the requested action silently did nothing.
- `gh stack` exits 2 both for a branch that is not in a stack and for a directory that is not a Git repository, so exit 2 alone cannot mean "local tracking missing".
- `gh` exits 4 with no "auth login" hint when no token is set, so the text-only auth classifier missed it.

## Decision

Add a live suite that runs the real binaries against long-lived fixture pull requests in this repository, described in [live validation](../architecture/07-live-validation.md).

- **Fixture over ephemeral stacks.** Fixture PRs, `fixture-start/*` tags, and a reset script give deterministic starting states without per-run PR creation. The suite creates nothing and needs no merge rights.
- **Tags reset state.** Branch drift is undone by force-pushing a branch to its tag. A merged stack cannot be reset, so it is read-only.
- **Rulesets protect the fixture.** Branch deletion and tag updates are blocked for everyone but admins. CI skips `fixture/*` pull requests.
- **Opt-in and off the PR path.** Tests run only with `STACK_STAT_LIVE=1`. CI runs them on a schedule and by manual dispatch with a serializing `concurrency` group, because they mutate shared remote state, depend on rate limits, and need a write token that fork PRs must not reach.
- **Pinned and latest toolchains.** The nightly run tests a pinned known-good `gh` and `gh-stack` pair and the latest releases of both. A latest-only failure is upstream drift; the pinned pair moves only after the suite passes against it. A failed scheduled run opens or updates a tracking issue.
- **Classify by text and code together.** Exit 2 maps to `LocalTrackingMissing` only with `is not part of a stack`; exit 4 maps to `Auth`; a first-use auto-install is retried once.
- **No PTY harness.** The `AGENTS.md` rule against PTY tests and visual snapshots stands. Dashboard behavior stays covered by `TestBackend`; the live suite targets the adapter boundary where the real binaries matter.
- **Existing fake tests stay.** They run offline on every pull request and cover logic the live suite does not. None is deleted; the classifier tests were rewritten to the observed output.

## Consequences

- A change in `gh` or `gh stack` output, exit codes, or GraphQL shape that breaks the adapter fails the scheduled run within a day, and a maintainer can reproduce it with `make live-test`.
- Fixture PRs appear in this repository's pull request list permanently.
- Runs take minutes and can fail from GitHub outages, rate limits, or someone editing a fixture PR.
- Coverage gaps (pagination over 100 entries, fork push, version matrix, real TUI) are listed in the living page and are not claimed as covered.

## Alternatives considered

- **Ephemeral stacks per run.** Isolated and self-cleaning, but slower, requiring a write token with PR creation, and leaves orphans when a run is cancelled.
- **Recorded responses behind a fake `gh`.** Fast and offline, but cannot detect that GitHub or `gh` changed; kept as a possible base layer.
- **PTY or VHS driver for the dashboard.** Rejected for now; it would reverse the `AGENTS.md` rule and adds flakiness for little extra coverage beyond the adapter checks.
