# stack-stat

A Rust terminal dashboard for [GitHub stacked PRs](https://github.com/github/gh-stack). It follows the checked-out worktree branch and shows stack layers, draft/ready state, CI checks, reviews, and commit-specific local checks. The selected row and checked-out branch remain distinct.

## Prerequisites

Install Git and an authenticated [GitHub CLI](https://cli.github.com/), then install the [`gh stack` extension](https://github.com/github/gh-stack):

```sh
gh auth login
gh extension install github/gh-stack
```

## Install

When a release is available, the macOS/Linux installer downloads a checksum-verified binary:

```sh
bash <(gh api repos/tcamise-gpsw/stack-stat/contents/scripts/install.sh -H 'Accept: application/vnd.github.raw')
```

Windows users can run the [PowerShell installer](scripts/install.ps1). Alternatively, build locally with `cargo build --release`; the binary is `target/release/stack-stat`. Run `stack-stat update` to check for and install a newer stable release; it never updates automatically.

## Use

From a Git worktree whose branch belongs to a `gh stack` stack, run `stack-stat`. Use `j/k` to select a layer and Enter to open details; `J/K`, Page Up/Down, or the mouse wheel scroll the detail pane. The mouse wheel over the table moves branch selection. Press `?` for the authoritative keybinding list and `q` to quit. Mutating stack operations ask for confirmation.

External agents and scripts can publish local results with `stack-stat report lint pass`; see [local checks](docs/user/local-checks.md) and [configuration](docs/user/configuration.md). [User documentation](https://tcamise-gpsw.github.io/stack-stat/) covers the full UI and troubleshooting. Developers should read [AGENTS.md](AGENTS.md), the [contributor guide](CONTRIBUTING.md), and [architecture](docs/architecture/01-l1-context.md).
