<p align="center">
  <img src="assets/hero.png" width="820" alt="StackStat: GitHub PR stacks, at a glance">
</p>

<p align="center">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-stable-DEA584?style=flat&colorA=222222&logo=rust&logoColor=white" alt="Rust"></a>
  <a href="https://ratatui.rs"><img src="https://img.shields.io/badge/TUI-ratatui-222222?style=flat&colorA=222222" alt="Built with ratatui"></a>
  <a href="https://github.com/tcamise-gpsw/stack-stat/releases"><img src="https://img.shields.io/badge/release-latest-4CAF50?style=flat&colorA=222222" alt="Releases"></a>
  <a href="https://github.com/tcamise-gpsw/stack-stat"><img src="https://img.shields.io/badge/source-GitHub-222222?style=flat&logo=github" alt="GitHub"></a>
</p>

# stack-stat

See every layer of the stack containing your worktree's checked-out branch, and know at a glance whether each one is ready to merge and, if not, what is blocking it. The header always identifies the actual worktree branch, even when an agent switches branches while the dashboard is open.

Each layer carries a verdict glyph: `✔` ready, `◷` waiting on CI, a review, or a local result, and `✖` blocked. The header names the next layer to merge with a bullet per issue, and the always-open detail pane lists failing checks first. Here is a short tour:

<p align="center">
  <img src="assets/demo.gif" width="720" alt="Demo of the stack-stat dashboard: selecting layers, reading checks and reviews, and scrolling details">
</p>

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

- **Header:** repository, trunk, and number of layers; the checked-out worktree branch and age of the last GitHub refresh; and, on larger terminals, a pie chart of layer statuses beside two lines: `Next:` names the lowest open layer (stacks merge bottom-up) followed by one bullet per issue, colored red when it blocks and yellow when it is only waiting (or `ready to merge`), and `Blocked by:` counts layers per category (checks, reviews, threads, rebase, local, draft). On smaller terminals the header shows colored counts of ready (`✔`), waiting (`◷`), and blocked (`✖`) layers instead. A spinner means data is being fetched; an overdue refresh is highlighted.
- **Table:** layers hang off the trunk on a git-tree style spine; each layer is two lines with its colored verdict glyph as the node, then the branch, PR number, lifecycle (draft, open, merged, queued), checks, reviews, and local reports, then the PR title. The animated, pulsing row is the selected layer shown in the detail pane; `*` in the left column marks the branch checked out in this worktree. The verdict glyph and the branch color answer "is this ready?": `✔` means ready, not a draft, CI passing, no changes requested or review outstanding, no rebase needed, and declared local checks fresh and passing; `◷` means nothing is blocking but evidence is outstanding (running CI, pending review, missing local result); `✖` means a draft, failed CI, requested changes, rebase needed, or a failed or stale local result. Merged layers are dimmed in every column; queued layers are green, and a draft state is red. The Checks cell shows `✔` (green) when everything passed and `✖` (red) when something failed; while any check is queued or running, an animated spinner replaces that icon and keeps the color of the worst state, so it spins yellow normally and red when something has already failed, and the icon returns when the checks finish. It gives passed/total for checks that ran, excluding those skipped by the workflow or by [repository configuration](configuration.md). `—` means no result. The Threads column counts unresolved review threads: green `✔ 0` when all are resolved, yellow `◷ N` when some are open, red `✖ N` when open threads are configured to block (see [configuration](configuration.md)). The Rebase column appears only when a layer needs a rebase: `↻ below` (red) means it must be rebased onto the layer under it, and `↻ main` means the bottom open layer is behind the trunk (gray unless `rebase.onto_trunk` makes it blocking). The Local column appears only when local checks are declared or reported for the stack. The Reviews cell is red for changes requested, yellow when GitHub requires a review that is still outstanding (`required` when nobody has been asked, `pending` when someone has), and green when approved; when reviews are not required it stays gray and does not block the ready verdict.
- **Detail pane:** always open for the selected branch and starts on the worktree's checked-out branch. Checks are listed failing first, then running, passed, and skipped, with durations; reviews show a headline and each visible reviewer, then local reports and whether the branch needs a rebase. When the details are longer than the pane, a scrollbar on its right edge shows the position; scroll with Page Up/Down or the mouse wheel.
- **Footer:** key hints and operation results. Actions that change GitHub or the worktree prompt for `y`; any other key cancels.

| Key or mouse gesture | Effect |
|---|---|
| ↑/↓, Home/End | Select next/previous or first/last stack layer |
| Page Down/Up | Scroll the detail pane by page; branch selection stays put |
| Wheel over table / detail | Move selected branch / scroll detail lines |
| Click a table row | Select that branch |
| `o` | Open the selected PR in a browser |
| `c`, `p`, `S`, `m` | Confirm checkout, push, sync, or draft/ready toggle |
| `R` | Refresh all data sources |
| `/`, `s`, `b` | Filter branches, cycle sort order, show/hide bot reviews |
| `?`, `q`, Ctrl-C | Help, quit |

The in-app `?` overlay is the source of truth for keybindings. In filter mode, type a branch substring, press Enter to keep it, or Esc to clear it. `s` cycles stack order, ascending PR number, and worst-check-first status order. Git and local results refresh frequently; GitHub refreshes more often while CI is active. Network failures preserve the last successful GitHub data.

Agents, hooks, and scripts can report commit-specific [local checks](local-checks.md). See [configuration](configuration.md) for CI filters, reviewer preferences, and polling, or [troubleshooting](troubleshooting.md) for error states.
