# Troubleshooting

## The dashboard does not open

| Message | Action |
|---|---|
| `gh CLI not found` | Install [GitHub CLI](https://cli.github.com/) and check `gh --version`. |
| `gh not authenticated` | Run `gh auth login`, then `gh auth status`. |
| `gh-stack not installed` | Run `gh extension install github/gh-stack`; verify with `gh extension list`. |
| `not in a git repository` | Run from a worktree; `git rev-parse --show-toplevel` should print its root. |

`stack-stat version` does not need a repository or GitHub login. `stack-stat report` needs Git but does not need the `gh stack` extension.

## “Not in a stack”

The TUI follows the **checked-out worktree branch**. On trunk or a branch outside a `gh stack` stack it does not retain an earlier stack. Switch to an existing stack branch; only use `gh stack init` if you actually intend to create a new local stack. The dashboard never initializes one automatically.

## Data looks old or incomplete

The footer shows fetch failures; the last successful GitHub data remains visible during a network outage. `R` requests refresh. GitHub rate limiting pauses remote requests until the reset time (or the retry fallback) while local Git and check reports keep refreshing. If authentication expires, run `gh auth login` again. A PR number with no draft/check/review details may be inaccessible or deleted; the stack still supplies its identity.

Local checks apply to a commit SHA, not merely a branch name. If a report is **stale**, run the check again on the branch's current commit and issue a fresh [`stack-stat report`](local-checks.md). An expected check marked **missing** has not been reported for this commit. A malformed local JSON file is skipped with a warning rather than hiding other reports. Reporting that check again for the same commit replaces the damaged file; `stack-stat report --clear --branch <name>` removes all entries for that branch.

## Mutations and rebase conflicts

`c`, `p`, `S`, and `m` require a `y` confirmation. Only one operation runs at once. A `gh stack` conflict leaves the TUI open and reports the error; resolve it with `gh stack rebase --continue` or `gh stack rebase --abort` in a terminal after inspecting the repository state. stack-stat never silently resolves conflicts.

## Releases and terminal display

Before the first stable release, `stack-stat update` exits with `no stable release found`. The version check inside the TUI is background-only and does not prevent startup. If colors look wrong, set `COLORTERM=truecolor` in a capable terminal or let `TERM` select a 256-color palette; basic 16-color terminals still show status icons and a bold active-branch pulse. If the terminal becomes narrow, review and local columns collapse; resize or open the detail pane to see full names. Overflowing details scroll with `J/K`, Page Up/Down, or the mouse wheel over the pane.

See [configuration](configuration.md) for polling and filter settings.
