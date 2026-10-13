# Contributing to stack-stat

## First-time setup

Install Rust, git, and an authenticated GitHub CLI with the `gh stack` extension. Then run `make dev-setup` to check prerequisites and install the versioned pre-commit hook.

## Day-to-day

```sh
make help
make format
make lint
make tests
make preflight
```

For build, focused test, and commit commands, see [AGENTS.md](AGENTS.md). System structure and significant decisions belong in [architecture](docs/architecture/) and [ADRs](docs/adr/README.md).

## Demo

`make demo` runs the dashboard against a fictional stack in [the demo example](crates/stack-stat-tui/examples/demo.rs). In-memory fake providers replace Git, GitHub, and the status store, so it needs no repository or stack; only the dashboard's background release check still calls `gh`, and its failure is silent. The stack covers each layer verdict and issue category. `make demo-gif` re-records the README and docs-site gif with [VHS](https://github.com/charmbracelet/vhs) from [the tape](docs/user/assets/demo.tape). When the dashboard's appearance changes, re-record it and update the example if a new state needs to appear.

## Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/). The authoritative types and scopes are in [`.gitmessage`](.gitmessage). Update documentation alongside the change.

## Repository setup

The release-please workflow uses the `GH_ACCESS_TOKEN` Actions repository secret. Its value is a fine-grained personal access token owned by `tcamise-gpsw`, limited to this repository. Give it **Contents**, **Pull requests**, and **Issues** repository permissions at Read and write; Metadata read-only is automatic. Rotate it before expiration. Set or rotate it through `gh secret set GH_ACCESS_TOKEN --repo tcamise-gpsw/stack-stat` using stdin or the interactive prompt, never by putting the value in a command argument, repository file, or chat. Only the secret name can be listed with `gh secret list --repo tcamise-gpsw/stack-stat`.

The site deploys using GitHub Pages configured with **Settings → Pages → Source: GitHub Actions**. Keep both settings in place for automated release and docs deployment.
