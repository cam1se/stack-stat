# Configuration

The repository file `.stack-stat.toml` lives at the current worktree root. It declares which GitHub checks matter in the display, which local checks are expected, and bot reviewer names. No configuration file is required.

```toml
[checks.skip]
names = ["secrets-scan"]
patterns = ["codecov/*"]

[checks.local]
lint = { description = "cargo clippy --workspace --all-targets --locked -- -D warnings" }
test = { description = "cargo test --workspace --locked --no-fail-fast" }

[reviews]
bots = ["ci-bot"]
show_bots = false
gate_on_unresolved_threads = false

[rebase]
onto_trunk = false

[poll]
git = 2
store = 2
github = 30
github_active = 15
```

`checks.skip.names` matches exact GitHub check names; `patterns` matches the **whole** name, with `*` for any span and `?` for one character. Excluded checks remain visible in the detail pane as "skipped (config)" but do not count toward passed/total. `checks.local` names expected manual reports; each description is display text, **not a command the TUI executes**. Missing and stale results remain visible until agents or scripts report a result for the current commit. `reviews.gate_on_unresolved_threads` (default `false`) decides whether unresolved review threads block the ready verdict: when `true`, any unresolved thread makes the layer blocked (`✖`); when `false`, the Threads column still shows the count in yellow but the verdict ignores it. Only the first 100 threads of each PR are counted. This is a repository setting; personal config cannot change it. `rebase.onto_trunk` (default `false`) decides whether the bottom open layer being behind the trunk blocks the ready verdict. Rebases between stack layers always block; trunk drift is shown dimly in the Rebase column but ignored by the verdict unless this is `true`. This is a repository setting. The `b` key toggles bot visibility during a session. GitHub's bot author type is recognized automatically; `reviews.bots` supplements it.

The poll values are positive whole seconds. Defaults: local Git and store every 2 seconds, GitHub every 30 seconds while idle and 15 seconds while checks are queued or running. `R` requests an immediate refresh. GitHub rate limiting temporarily suspends remote polling but local data continues. No animation toggle is needed; animations are always enabled.

## Personal display preferences

Place user-only settings at `$XDG_CONFIG_HOME/stack-stat/config.toml` when `XDG_CONFIG_HOME` is set, at `%APPDATA%\stack-stat\config.toml` on Windows otherwise, or `~/.config/stack-stat/config.toml` on Unix otherwise:

```toml
[poll]
github = 60

[display]
reviews_default = "expanded"
```

`reviews_default` is `collapsed` (default) or `expanded` (per-reviewer inline on wide terminals). The detail pane always shows individual visible reviewers. Personal `[poll]` values override repository poll values. Repository `[checks.*]` and bot settings are authoritative: user attempts to set them are ignored with a warning, not used to un-skip CI or alter declared checks. Unknown keys, malformed TOML, or zero-second intervals fail configuration loading with a path or key diagnostic. Fix the file before reopening the TUI.

See [local checks](local-checks.md) for the `report` command and [the dashboard](index.md) for keybindings.
