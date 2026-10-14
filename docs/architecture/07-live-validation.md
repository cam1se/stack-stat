# 07 Live validation

The fake-based suites prove stack-stat's logic; they cannot prove that `gh` and `gh stack` still print and exit the way the adapter assumes. The live suite in [`live_gh.rs`](../../crates/stack-stat-core/tests/live_gh.rs) runs the real binaries against fixture pull requests on GitHub. The choice and its trade-offs are recorded in [ADR-0004](../adr/0004-live-fixture-validation.md).

## Run it

```sh
make live-test            # needs gh auth with push access to the fixture repository
make live-fixture-reset   # restore fixture branches to their start tags
```

Tests skip silently unless `STACK_STAT_LIVE=1`; `make live-test` sets it. `STACK_STAT_FIXTURE_REPO` and `STACK_STAT_FIXTURE_FORK` override the fixture repository (default `cam1se/stack-stat`) and the fork that holds the fork PRs (default `tcamise-gpsw/stack-stat`). The suite needs `git`, `gh`, and network access; the `gh stack` extension is installed by `gh` itself on first use.

## Fixture

The fixture lives in this repository under the `fixture/` branch prefix. Branches are protected from deletion and the `fixture-start/*` and `fixture-conflict/*` tags from update or deletion by repository rulesets; admins bypass them. CI skips pull requests whose head branch starts with `fixture/`.

|Stack|Pull requests|Trunk|Role|
|---|---|---|---|
|`s1`|three stacked drafts on `fixture/s1-a`, `-b`, `-c`|`main`|Reads, link, push, sync, draft toggle|
|`s2`|two stacked drafts on `fixture/s2-a`, `-b`|`fixture/trunk`|Rebase conflict: `scripts/live-fixture.sh conflict` moves trunk to a commit editing the same file|
|`s3`|two merged PRs, branches deleted|`fixture/trunk`|Merged stack reads; read-only, cannot be reset|
|`s4`|one unstacked upstream PR on `fixture/s4-same`|`main`|Single PR on the default branch|
|`s5`|one unstacked draft on `fixture/s5-single`|`fixture/single-base`|Single PR whose base is not the default branch; draft toggle|
|`s6`|branch `fixture/s6-nopr`, pushed, no PR|none|No-PR branch; trunk is the default branch|
|`s7`|one closed, unmerged PR|`fixture/single-base`|Single closed PR; read-only|
|`s8`|one merged PR|`fixture/single-base`|Single merged PR; read-only, cannot be reset|
|fork|fork PRs from `fixture/s1-c` and `fixture/s4-same`|`main`|Same head branch name as an upstream PR, different head owner|

[`scripts/live-fixture.sh`](../../scripts/live-fixture.sh) implements `status`, `reset`, and `conflict`. `reset` force-pushes each branch to its `fixture-start/<name>` tag and restores draft state. Read-only tests hold a shared fixture lock; a mutating test holds it exclusively, resets the fixture before it starts, and resets again when it ends, including when it panics, so no test reads a half-reset fixture. The workflow adds a final reset step and a `concurrency` group so two runs never touch the fixture at once.

Tag a new baseline with `git tag -f fixture-start/<branch> <sha>` only after temporarily lifting the tag ruleset.

## Pinned behavior

Observed with `gh` 2.101.0 and `gh-stack` v0.1.1:

|Situation|Observed|Adapter result|
|---|---|---|
|`gh stack` with the extension absent|`gh` installs it, prints `Successfully installed github/gh-stack`, exits 0 without running the subcommand|Retry the command once|
|Push or sync in a repository where the branch is untracked|exit 2, `current branch "…" is not part of a stack`|`LocalTrackingMissing`|
|Any `gh stack` command outside a Git repository|exit 2, `not a git repository`|`Command { code: 2, .. }`|
|`gh` with no token|exit 4, `To use GitHub CLI in automation, set the GH_TOKEN`|`Auth`|
|Sync against a conflicting trunk|stack-stat treats exit codes 3 and 7 as a conflict; the live test confirms the result, not which code|`RebaseConflict`|

## Coverage and gaps

Covered live: owner/name resolution; stacked, merged-stack and single-mode discovery (unstacked, non-default base, no PR, closed, merged) with PR details; same-name fork PR; link (`gh stack checkout`), push, sync, and draft toggle for stacked and single PRs; sync conflict; missing tracking, repository, extension, and authentication.

Not covered:

- **Discovery over 100 stack entries.** Needs a 100-PR stack; pagination remains fixture-tested.
- **Fork clone pushing to its own fork.** The fork PR is read with a fork checkout, but push and sync from a fork are not exercised.
- **Exact conflict exit code.** The test asserts the classified result, not whether `gh stack` returned 3 or 7.
- **Real dashboard rendering.** There is no headless stack printer; the TUI is still covered through `TestBackend` and fake providers. Revisit if a non-interactive resolved-stack command is added.
- **Older `gh` versions.** The workflow tests one pinned known-good pair and the latest releases; versions in between are not tested.
- **`s3` and fork PR drift.** If someone closes a fixture PR or deletes a fork branch, tests fail until the fixture is rebuilt by hand.

## Credentials

The workflow uses the `GH_ACCESS_TOKEN` repository secret, falling back to the workflow token. The token must force-push `fixture/*` branches, toggle draft state on the `s1`/`s2` PRs, and open issues; it never needs to merge. The workflow runs on a schedule and on manual dispatch only, never on pull requests, so a fork PR cannot reach the token.

## Detecting version drift

The workflow runs a two-job matrix nightly (06:17 UTC) and on dispatch. `pinned` uses the versions the suite was validated against (`gh` 2.101.0, `gh-stack` v0.1.1); `latest` installs the newest `gh` release and the newest extension. A failure in `latest` with `pinned` passing means an upstream change breaks stack-stat; a failure in both points at the fixture or the adapter. Raise the pinned versions only after the suite passes against them. A failed scheduled run opens or updates the issue titled "Live gh drift detected". Each job records the exact versions in its step summary.
