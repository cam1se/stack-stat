# stack-stat

A terminal dashboard for GitHub stacked pull requests. It displays the stack of the current branch, pull request status, GitHub checks, reviews, and local check results.

## Prerequisites

Install Git, the authenticated [GitHub CLI](https://cli.github.com/), and [gh stack](https://github.com/github/gh-stack):

```sh
gh auth login
gh extension install github/gh-stack
```

## Install

On macOS or Linux, run the [installer](https://github.com/tcamise-gpsw/stack-stat/blob/main/scripts/install.sh). On Windows, use [PowerShell installer](https://github.com/tcamise-gpsw/stack-stat/blob/main/scripts/install.ps1). The installer verifies the release archive checksum before placing the binary on PATH.

User instructions will expand as the dashboard is implemented. For now, the design and executable plan live in the source repository.
