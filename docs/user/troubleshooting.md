# Troubleshooting

## The dashboard does not open

| Message | Action |
|---|---|
| `gh CLI not found` | Install [GitHub CLI](https://cli.github.com/) and check `gh --version`. |
| `gh not authenticated` | Run `gh auth login`, then `gh auth status`. |
| `not in a git repository` | Run from a worktree; `git rev-parse --show-toplevel` should print its root. |

`stack-stat version` does not need a repository or GitHub login. `stack-stat report` needs Git but does not need GitHub authentication. The dashboard needs authenticated `gh`, but neither reading stacks nor reporting checks requires the `gh stack` extension.

## “Not in a stack”

The TUI follows the **checked-out worktree branch** and reads its PR's stack from GitHub, not local `gh stack` tracking. A branch without a PR, or a PR with no GitHub stack, shows this view and clears any earlier stack. Open the branch's PR and stack it on GitHub, or switch to an existing stacked PR branch. A fresh clone or worktree can read an existing stack without initializing or linking it locally.

## Data looks old or incomplete

The footer shows fetch failures; the last successful GitHub data remains visible during a network outage. `R` requests refresh. GitHub rate limiting pauses remote requests until the reset time (or the retry fallback) while local Git and check reports keep refreshing. If authentication expires, run `gh auth login` again. A PR number with no draft/check/review details may be inaccessible or deleted; the stack still supplies its identity.

Local checks apply to a commit SHA, not merely a branch name. If a report is **stale**, run the check again on the branch's current commit and issue a fresh [`stack-stat report`](local-checks.md). An expected check marked **missing** has not been reported for this commit. A malformed local JSON file is skipped with a warning rather than hiding other reports. Reporting that check again for the same commit replaces the damaged file; `stack-stat report --clear --branch <name>` removes all entries for that branch.

The muted suffix beside the `*` worktree marker is local sync information only: `↑` means commits have not been pushed, `↓` means the remote-tracking ref has commits to incorporate, `⇅` means both sides have unique commits, and `?` means no upstream or `origin/<branch>` ref exists. It does not affect the layer verdict. Push, fetch, rebase, or configure an upstream as appropriate.

## Mutations and rebase conflicts

`c`, `p`, `S`, and `m` require a `y` confirmation. Only one operation runs at once. A `gh stack` conflict leaves the TUI open and reports the error; resolve it with `gh stack rebase --continue` or `gh stack rebase --abort` in a terminal after inspecting the repository state. A sync that prints `Sync aborted` despite exiting 0 means local and remote stack chains diverge; stack-stat treats it as an abort, not as a successful sync. Inspect the two chains before retrying. stack-stat never silently resolves conflicts.

If checkout, push, or sync reports that `gh stack` is unavailable, run `gh extension install github/gh-stack` and verify with `gh extension list`. The extension is needed only for these actions; its absence does not prevent the dashboard from opening.

Push and sync need local stack tracking. When it is missing, stack-stat asks for confirmation before running `gh stack checkout <pr-number-or-branch>` to link it, then continues the requested action. Checkout can change the worktree, so inspect the target in the prompt. Declining does not link or retry and leaves a cancellation message. No read or refresh links a stack automatically.

## Releases and terminal display

Before the first stable release, `stack-stat update` exits with `no stable release found`. The version check inside the TUI is background-only and does not prevent startup. If colors look wrong, set `COLORTERM=truecolor` in a capable terminal or let `TERM` select a 256-color palette; basic 16-color terminals still show verdict glyphs and a bold pulse on the selected row. If the terminal becomes narrow, review and local columns collapse; resize to see full names. Overflowing details scroll with Page Up/Down or the mouse wheel over the pane.

See [configuration](configuration.md) for polling and filter settings.
