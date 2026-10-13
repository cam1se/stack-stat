![StackStat: GitHub PR stacks, at a glance](assets/hero.png)

# stack-stat

See every layer of the stack containing your worktree's checked-out branch. The table shows branch and PR identity, draft/ready/merged/queued state, CI checks, reviews, and local reports. The header always identifies the actual worktree branch, even when an agent switches branches while the dashboard is open.

## Setup

Install Git, an authenticated [GitHub CLI](https://cli.github.com/), and [gh stack](https://github.com/github/gh-stack):

```sh
gh auth login
gh extension install github/gh-stack
```

From a worktree on a stack branch, run `stack-stat`. The tool follows **only the current branch's stack**; on a branch outside a stack it shows "Not in a stack" rather than guessing. GitHub CLI and the extension are required at startup.

## Install and update

After a stable release is published, macOS/Linux users can run the checksum-verifying installer:

```sh
bash <(gh api repos/tcamise-gpsw/stack-stat/contents/scripts/install.sh -H 'Accept: application/vnd.github.raw')
```

Windows users can run [install.ps1](https://github.com/tcamise-gpsw/stack-stat/blob/main/scripts/install.ps1) in PowerShell. `STACK_STAT_VERSION=vX.Y.Z` pins a stable release; `INSTALL_DIR` changes the destination. Both installers verify `checksums.sha256` before installing.

`stack-stat version` prints the version and target triple. `stack-stat update` checks GitHub Releases for a newer stable binary, verifies its checksum, and replaces the installed executable. It does not update on its own; the TUI only displays an availability notice. Until the first release exists, `update` exits with `no stable release found`.

## Read the dashboard

- **Header:** repository, trunk, and number of layers; the checked-out worktree branch and age of the last GitHub refresh; and a count of layers ready (`✔`), waiting (`…`), or blocked (`✖`), with a pie chart of the same statuses on larger terminals. A spinner means data is being fetched; an overdue refresh is highlighted.
- **Table:** layers hang off the trunk on a git-tree style spine; each layer is two lines with its colored verdict glyph as the node, then the branch, PR number, lifecycle (draft, open, merged, queued), checks, reviews, and local reports, then the PR title. `►` selects a row; `←wt` identifies the checked-out branch. The verdict glyph and the branch color answer "is this ready?": `✔` means ready, not a draft, CI passing, no changes requested or review outstanding, no rebase needed, and declared local checks fresh and passing; `…` means nothing is blocking but evidence is outstanding (running CI, pending review, missing local result); `✖` means a draft, failed CI, requested changes, rebase needed, or a failed or stale local result. Merged layers are muted; queued layers are green. The Checks cell gives passed/total for checks that ran, excluding those skipped by the workflow or by [repository configuration](configuration.md). `—` means no result. The Reviews cell is red for changes requested, yellow when GitHub requires a review that is still outstanding (`required` when nobody has been asked, `pending` when someone has), and green when approved; when reviews are not required it stays gray and does not block the ready verdict.
- **Detail pane:** always open for the selected branch and starts on the worktree's checked-out branch. Checks are listed failing first, then running, skipped, and passed, with durations; reviews show a headline and each visible reviewer, then local reports and whether the branch needs a rebase. Overflowing details have a continuation marker and can be scrolled.
- **Footer:** key hints and operation results. Actions that change GitHub or the worktree prompt for `y`; any other key cancels.

| Key or mouse gesture | Effect |
|---|---|
| `j/k`, ↑/↓, `g/G` | Select next/previous or first/last stack layer |
| `J/K`, Page Down/Up | Scroll details by line or page; branch selection stays put |
| Wheel over table / detail | Move selected branch / scroll detail lines |
| Click a table row | Select that branch |
| `o` | Open the selected PR in a browser |
| `c`, `p`, `S`, `m` | Confirm checkout, push, sync, or draft/ready toggle |
| `R` | Refresh all data sources |
| `/`, `s`, `b` | Filter branches, cycle sort order, show/hide bot reviews |
| `?`, `q`, Ctrl-C | Help, quit |

The in-app `?` overlay is the source of truth for keybindings. In filter mode, type a branch substring, press Enter to keep it, or Esc to clear it. `s` cycles stack order, ascending PR number, and worst-check-first status order. Git and local results refresh frequently; GitHub refreshes more often while CI is active. Network failures preserve the last successful GitHub data.

Agents, hooks, and scripts can report commit-specific [local checks](local-checks.md). See [configuration](configuration.md) for CI filters, reviewer preferences, and polling, or [troubleshooting](troubleshooting.md) for error states.
