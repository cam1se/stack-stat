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

## Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/). The authoritative types and scopes are in [`.gitmessage`](.gitmessage). Update documentation alongside the change.
