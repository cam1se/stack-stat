<p align="center">
  <img src="docs/user/assets/hero.png" width="820" alt="StackStat: GitHub PR stacks, at a glance">
</p>

<p align="center">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-stable-DEA584?style=flat&colorA=222222&logo=rust&logoColor=white" alt="Rust"></a>
  <a href="https://ratatui.rs"><img src="https://img.shields.io/badge/TUI-ratatui-222222?style=flat&colorA=222222" alt="Built with ratatui"></a>
  <a href="https://github.com/cam1se/stack-stat/actions/workflows/ci.yml"><img src="https://img.shields.io/badge/CI-GitHub_Actions-2088FF?style=flat&logo=githubactions&logoColor=white" alt="CI"></a>
  <a href="https://github.com/cam1se/stack-stat/releases"><img src="https://img.shields.io/badge/release-latest-4CAF50?style=flat&colorA=222222" alt="Releases"></a>
  <a href="https://cam1se.github.io/stack-stat/"><img src="https://img.shields.io/badge/docs-gh--pages-4CAF50?style=flat&colorA=222222" alt="Docs"></a>
</p>

A Rust terminal dashboard for [GitHub stacked PRs](https://github.com/github/gh-stack), single PRs, and branches before PR creation. It answers two questions at a glance: <strong>is each PR ready to merge?</strong> and, if not, <strong>what is blocking it?</strong> It follows the checked-out worktree branch, so it stays accurate while an agent hops between branches.

> [!TIP]
> Full documentation, including configuration, local checks, and troubleshooting, is at **[cam1se.github.io/stack-stat](https://cam1se.github.io/stack-stat/)**.

Open PRs use verdict glyphs: `✔` ready to merge, `◷` still waiting on CI, a review, or a local result, and `✖` blocked. Merged and queued PRs use separate lifecycle glyphs. The `Next to merge:` header names the next PR and lists its issues, and the detail pane for the selected branch is always open, with failing checks first. A branch with no PR shows local checks and a `no PR yet` hint, not a ready-to-merge PR. Here it is in action:

<p align="center">
  <img src="docs/user/assets/demo.gif" width="840" alt="Demo of the stack-stat dashboard: selecting layers, reading checks and reviews, and scrolling details">
</p>

## Prerequisites

Install Git and an authenticated [GitHub CLI](https://cli.github.com/):

```sh
gh auth login
```

The dashboard reads stack membership directly from GitHub, so it works without local stack tracking. Install the [`gh stack` extension](https://github.com/github/gh-stack) only to check out, push, or sync stacks: `gh extension install github/gh-stack`.

## Install

> [!NOTE]
> One-time step. Keep it current afterwards with `stack-stat update`.

The macOS/Linux installer downloads a checksum-verified binary:

```sh
bash <(gh api repos/cam1se/stack-stat/contents/scripts/install.sh -H 'Accept: application/vnd.github.raw')
```

Windows users can run the [PowerShell installer](scripts/install.ps1). Alternatively, build locally with `cargo build --release`; the binary is `target/release/stack-stat`. Run `stack-stat update` to check for and install a newer stable release; it never updates automatically.

## Use

From a Git worktree, run `stack-stat`. It shows the current branch's GitHub stack, a single unstacked PR, or a no-PR branch row. The detail pane starts on the worktree's branch. Use ↑/↓ to select a row; Page Up/Down or the mouse wheel scroll the detail pane. Press `?` for the authoritative keybinding list and `q` to quit. Available mutations ask for confirmation; stacked push and sync ask before linking missing local tracking. Single-mode push uses plain `git push`, while single-mode checkout and sync are disabled. See the [user guide](docs/user/index.md#setup) for trunk selection and mode-specific actions.

## Configuration

No configuration is required. To tune the display, add a `.stack-stat.toml` at the worktree root: skip noisy GitHub checks, declare the local checks you expect, name bot reviewers, and adjust poll intervals.

```toml
[checks.skip]
patterns = ["codecov/*"]

[checks.local]
lint = { description = "cargo clippy --workspace --all-targets --locked -- -D warnings" }
```

Personal preferences such as poll intervals and review layout go in `~/.config/stack-stat/config.toml`. See [configuration](docs/user/configuration.md) for every option, defaults, and file locations.

## Local checks

Checks that run on your machine, such as lint and tests, can be recorded against the branch and commit they ran on, so the dashboard shows them next to CI. An agent or script reports a result with `stack-stat report`:

```sh
stack-stat report lint pass
stack-stat report test fail --message 'three assertions failed'
stack-stat report --list
```

A result goes stale when the branch tip moves, so an old pass is never shown as current. The description in `[checks.local]` is display text only; stack-stat never runs the command. See [local checks](docs/user/local-checks.md) for flags, JSON output, and storage.

## More

[User documentation](https://cam1se.github.io/stack-stat/) covers the full UI and troubleshooting. Developers should read [AGENTS.md](AGENTS.md), the [contributor guide](CONTRIBUTING.md), and [architecture](docs/architecture/01-l1-context.md).
