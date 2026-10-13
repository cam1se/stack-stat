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

- **Header:** stack order from trunk to top, checked-out worktree branch, and age of the last GitHub refresh. A spinner means data is being fetched; an overdue refresh is highlighted.
- **Table:** `►` selects a row; `←wt` identifies the checked-out branch. The Checks cell gives passed/total for checks not excluded by [repository configuration](configuration.md). `—` means no result; draft, ready, merged, and queued states stay separate. The Local column distinguishes pass, fail, missing, and stale evidence.
- **Detail pane:** Enter or Tab opens it for the selected branch. It lists each CI check (including configured skips), individual human and bot reviewers, requested reviews, local reports, and whether the branch needs a rebase. Overflowing details have a continuation marker and can be scrolled.
- **Footer:** key hints and operation results. Actions that change GitHub or the worktree prompt for `y`; any other key cancels.

| Key or mouse gesture | Effect |
|---|---|
| `j/k`, ↑/↓, `g/G` | Select next/previous or first/last stack layer |
| Enter or Tab | Toggle selected branch details |
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
