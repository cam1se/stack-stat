<p align="center">
  <img src="docs/user/assets/hero.png" width="820" alt="StackStat: GitHub PR stacks, at a glance">
</p>

<p align="center">
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-stable-DEA584?style=flat&colorA=222222&logo=rust&logoColor=white" alt="Rust"></a>
  <a href="https://ratatui.rs"><img src="https://img.shields.io/badge/TUI-ratatui-222222?style=flat&colorA=222222" alt="Built with ratatui"></a>
  <a href="https://github.com/tcamise-gpsw/stack-stat/actions/workflows/ci.yml"><img src="https://img.shields.io/badge/CI-GitHub_Actions-2088FF?style=flat&logo=githubactions&logoColor=white" alt="CI"></a>
  <a href="https://github.com/tcamise-gpsw/stack-stat/releases"><img src="https://img.shields.io/badge/release-latest-4CAF50?style=flat&colorA=222222" alt="Releases"></a>
  <a href="https://tcamise-gpsw.github.io/stack-stat/"><img src="https://img.shields.io/badge/docs-gh--pages-4CAF50?style=flat&colorA=222222" alt="Docs"></a>
</p>

A Rust terminal dashboard for [GitHub stacked PRs](https://github.com/github/gh-stack) that answers two questions at a glance: <strong>is each layer ready to merge?</strong> and, if not, <strong>what is blocking it?</strong> It follows the checked-out worktree branch, so it stays accurate while an agent hops between branches.

> [!TIP]
> Full documentation, including configuration, local checks, and troubleshooting, is at **[tcamise-gpsw.github.io/stack-stat](https://tcamise-gpsw.github.io/stack-stat/)**.

Open layers use verdict glyphs: `✔` ready to merge, `◷` still waiting on CI, a review, or a local result, and `✖` blocked. Merged and queued layers use separate lifecycle glyphs. The header names the next layer to merge and lists its issues, and the detail pane for the selected layer is always open, with failing checks first. Here it is in action:

<p align="center">
  <img src="docs/user/assets/demo.gif" width="840" alt="Demo of the stack-stat dashboard: selecting layers, reading checks and reviews, and scrolling details">
</p>

## Prerequisites

Install Git and an authenticated [GitHub CLI](https://cli.github.com/), then install the [`gh stack` extension](https://github.com/github/gh-stack):

```sh
gh auth login
gh extension install github/gh-stack
```

## Install

> [!NOTE]
> One-time step. Keep it current afterwards with `stack-stat update`.

The macOS/Linux installer downloads a checksum-verified binary:

```sh
bash <(gh api repos/tcamise-gpsw/stack-stat/contents/scripts/install.sh -H 'Accept: application/vnd.github.raw')
```

Windows users can run the [PowerShell installer](scripts/install.ps1). Alternatively, build locally with `cargo build --release`; the binary is `target/release/stack-stat`. Run `stack-stat update` to check for and install a newer stable release; it never updates automatically.

## Use

From a Git worktree whose branch belongs to a `gh stack` stack, run `stack-stat`. The detail pane starts on the worktree's branch. Use ↑/↓ to select a layer; Page Up/Down or the mouse wheel scroll the detail pane. Press `?` for the authoritative keybinding list and `q` to quit. Mutating stack operations ask for confirmation.

External agents and scripts can publish local results with `stack-stat report lint pass`; see [local checks](docs/user/local-checks.md) and [configuration](docs/user/configuration.md). [User documentation](https://tcamise-gpsw.github.io/stack-stat/) covers the full UI and troubleshooting. Developers should read [AGENTS.md](AGENTS.md), the [contributor guide](CONTRIBUTING.md), and [architecture](docs/architecture/01-l1-context.md).
