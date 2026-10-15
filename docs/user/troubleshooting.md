# Troubleshooting

## The dashboard does not open

| Message | Action |
|---|---|
| `gh CLI not found` | Install [GitHub CLI](https://cli.github.com/) and check `gh --version`. |
| `gh not authenticated` | Run `gh auth login`, then `gh auth status`. |
| `not in a git repository` | Run from a worktree; `git rev-parse --show-toplevel` should print its root. |

`stack-stat version` does not need a repository or GitHub login. `stack-stat report` needs Git but does not need GitHub authentication. The dashboard needs authenticated `gh`, but neither reading stacked or single-mode data nor reporting checks requires the `gh stack` extension.

## Single PR or “no PR yet”

The TUI follows the **checked-out worktree branch** and reads its PR's stack from GitHub, not local `gh stack` tracking. A PR with a non-null GitHub stack uses stacked mode even when the stack has only one entry, and a fresh clone or worktree can read that stack without initializing or linking it locally. An unstacked PR appears as one entry with the PR's base branch as trunk and the usual CI, review, draft, and remote rebase details. A branch without a matching PR appears as one row with the repository's default branch as trunk, empty PR-only cells, and a `no PR yet` hint. Local checks still apply, including missing or stale reports; passing local checks do not make the branch a ready-to-merge PR or a `Next to merge` candidate. Open a PR when you want PR details; there is no need to create a stack just to use the dashboard. A PR closed without merging is shown as closed history and cannot be marked draft or ready.

Checkout (`c`) and sync (`S`) are disabled in single mode and report that status without running a stack command. Push (`p`) uses plain `git push` after confirmation and needs no extension or local stack tracking. If Git reports no upstream or another push configuration error, inspect the configured remote and branch in a terminal and set the upstream explicitly, for example with `git push --set-upstream <remote> <branch>`. stack-stat does not choose an upstream or link a stack for you. Browser opening (`o`) and draft/ready toggling (`m`) work when a PR exists; they are unavailable on a no-PR row.

If you switch branches outside the dashboard, push, checkout, and sync wait for the next GitHub result instead of acting on the previous branch's mode; the status bar reports the refresh, and the key works again once it completes.

## “No attached branch” or “Default branch”

A detached or empty HEAD shows “No attached branch” because there is no current branch to display. Check out a branch to use the dashboard. The repository's default branch without its own PR shows “Default branch”, because there is nothing to compare it against; check out a feature branch to see its row.

## Data looks old or incomplete

The footer shows fetch failures; the last successful GitHub data remains visible during a network outage. `R` requests refresh. GitHub rate limiting pauses remote requests until the reset time (or the retry fallback) while local Git and check reports keep refreshing. If authentication expires, run `gh auth login` again. A PR number with no draft/check/review details may be inaccessible or deleted; the stack still supplies its identity.

Local checks apply to a commit SHA, not merely a branch name. If a report is **stale**, run the check again on the branch's current commit and issue a fresh [`stack-stat report`](local-checks.md). An expected check marked **missing** has not been reported for this commit. A malformed local JSON file is skipped with a warning rather than hiding other reports. Reporting that check again for the same commit replaces the damaged file; `stack-stat report --clear --branch <name>` removes all entries for that branch.

The muted suffix beside the `*` worktree marker is local sync information only: `↑` means commits have not been pushed, `↓` means the remote-tracking ref has commits to incorporate, `⇅` means both sides have unique commits, and `?` means no upstream or `origin/<branch>` ref exists. It does not affect the layer verdict. Push, fetch, rebase, or configure an upstream as appropriate.

## Mutations and rebase conflicts

Available mutations require a `y` confirmation. Only one operation runs at once. In stacked mode, a `gh stack` conflict leaves the TUI open and reports the error; resolve it with `gh stack rebase --continue` or `gh stack rebase --abort` in a terminal after inspecting the repository state. A sync that prints `Sync aborted` despite exiting 0 means local and remote stack chains diverge; stack-stat treats it as an abort, not as a successful sync. Inspect the two chains before retrying. stack-stat never silently resolves conflicts.

If stacked checkout, push, or sync reports that `gh stack` is unavailable, run `gh extension install github/gh-stack` and verify with `gh extension list`. The extension is needed only for these stacked actions; its absence does not prevent the dashboard from opening or a single-mode push from running.

Stacked push and sync need local stack tracking. When it is missing, stack-stat asks for confirmation before running `gh stack checkout <pr-number-or-branch>` to link it, then continues the requested action. Checkout can change the worktree, so inspect the target in the prompt. Declining does not link or retry and leaves a cancellation message. No read or refresh links a stack automatically.

## Releases and terminal display

Before the first stable release, `stack-stat update` exits with `no stable release found`. The version check inside the TUI is background-only and does not prevent startup. If colors look wrong, set `COLORTERM=truecolor` in a capable terminal or let `TERM` select a 256-color palette; basic 16-color terminals still show verdict glyphs and a bold pulse on the selected row. If the terminal becomes narrow, review and local columns collapse; resize to see full names. Overflowing details scroll with Page Up/Down or the mouse wheel over the pane.

See [configuration](configuration.md) for polling and filter settings.
