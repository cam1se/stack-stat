# 04 — Domain state and configuration

[stack-stat-core](../../crates/stack-stat-core/) separates data fetched from Git and GitHub from the view rendered in the terminal. The domain model describes stack order, the current worktree branch, each layer's PR lifecycle, CI status, reviews, and local checks. Provider responses are mapped into the model by the compositor; the TUI reads the resulting state without depending on JSON response shapes.

## Derived status

CI checks excluded by repository configuration remain visible in the PR detail view but do not count toward the row's passed/total summary. Failure takes precedence over running, which takes precedence over success; skipped-only checks remain distinct. Review summaries consider visible reviewers: changes requested outranks approval, which outranks an outstanding request. The bot toggle changes visibility without changing the original GitHub data. Missing local checks are configured expectations that were never reported; stale results refer to a previous branch commit.

## Configuration sources

The repository's `.stack-stat.toml` at the worktree root owns check filters, expected local checks, bot names, and default bot visibility. Personal preferences live in `$XDG_CONFIG_HOME/stack-stat/config.toml`, the platform user configuration directory, or `~/.config/stack-stat/config.toml`. The user file may override polling intervals and the default review-column density. It cannot override repository check rules or bot names; ignored attempts produce a warning. Unknown keys and zero-second intervals fail configuration loading with a diagnostic naming the file or interval.

Glob patterns in the repository check filter match the whole check name: `*` matches a span and `?` matches one character. These display rules do not change what GitHub reports.

See [system boundaries](02-l2-containers.md) for the crates and local data sources.
