# 04 — Domain state and configuration

[stack-stat-core](../../crates/stack-stat-core/) separates data fetched from Git and GitHub from the view rendered in the terminal. The domain model describes stack order, the current worktree branch, each layer's PR lifecycle, CI status, reviews, and local checks. Provider responses are mapped into the model by the compositor; the TUI reads the resulting state without depending on JSON response shapes.

## Composition

Successful Git, GitHub, and store snapshots are composed without I/O or clock reads. GitHub stack order supplies rows and PR identities; Git supplies the worktree's checked-out branch and current local SHAs. A PR number from stack metadata remains visible even if GitHub details are temporarily unavailable. A transport failure keeps the prior successful snapshot in the App, while an explicit not-in-stack result clears rows without guessing another stack.

For a local check, the current-SHA report wins; otherwise the newest older report is stale. Declared checks without a report render as missing, and a branch without a local ref never treats a stored result as fresh. Review requests without a submitted latest review become pending reviewer entries. CI skip patterns affect summaries, not underlying check visibility. Snapshot timestamps are carried through to the view so the TUI can show refresh age without reading the wall clock during rendering.

## Derived status

CI checks excluded by repository configuration remain visible in the PR detail view but do not count toward the row's passed/total summary. Failure takes precedence over running, which takes precedence over success; checks that completed as skipped or neutral are not counted in the total, so many conditional workflows do not make a finished PR look incomplete; skipped-only checks remain distinct. A workflow `STARTUP_FAILURE` and an unknown completed conclusion are treated as failures rather than falsely green. An unresolved-thread count is collected with each PR; it blocks the verdict only when the repository sets `reviews.gate_on_unresolved_threads`. Review summaries consider visible reviewers: changes requested outranks approval, which outranks an outstanding request. The bot toggle changes visibility without changing the original GitHub data. Missing local checks are configured expectations that were never reported; stale results refer to a previous branch commit.

## Configuration sources

The repository's `.stack-stat.toml` at the worktree root owns check filters, expected local checks, bot names, and default bot visibility. Personal preferences live in `$XDG_CONFIG_HOME/stack-stat/config.toml`, the platform user configuration directory, or `~/.config/stack-stat/config.toml`. The user file may override polling intervals and the default review-column density. It cannot override repository check rules or bot names; ignored attempts produce a warning. Unknown keys and zero-second intervals fail configuration loading with a diagnostic naming the file or interval.

Glob patterns in the repository check filter match the whole check name: `*` matches a span and `?` matches one character. These display rules do not change what GitHub reports.

See [system boundaries](02-l2-containers.md) for the crates and local data sources.
