# stack-stat

A terminal dashboard for stacked pull requests. See the [design](docs/specs/001-stack-stat-design.md) while the first implementation is under way.

## Prerequisites

Install Rust, Git, authenticated [GitHub CLI](https://cli.github.com/), and the [`gh stack` extension](https://github.com/github/gh-stack). Run `gh extension install github/gh-stack` after `gh auth login`.

## Build

Run `make dev-setup` and `cargo build`. The [contributor guide](CONTRIBUTING.md) covers development; [documentation conventions](docs/README.md) and [architecture](docs/architecture/01-l1-context.md) explain the structure.

## Install from a release

Once a release exists, the checksum-verifying installers can be run through `gh`:

```sh
bash <(gh api repos/tcamise-gpsw/stack-stat/contents/scripts/install.sh -H 'Accept: application/vnd.github.raw')
```

On Windows, invoke the [PowerShell installer](scripts/install.ps1). See the [user docs](docs/user/index.md) for setup and usage.
