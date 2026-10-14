# Developer convenience targets for stack-stat.
.DEFAULT_GOAL := help
.PHONY: help dev-setup setup-hooks format lint tests live-test live-fixture-check live-fixture-reset preflight rustdoc docs-serve demo demo-gif clean

help: ## List available targets
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "} {printf "  \033[36m%-24s\033[0m %s\n", $$1, $$2}'

dev-setup: ## Check prerequisites and install git hooks
	@command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found — install Rust from https://rustup.rs"; exit 1; }
	@command -v git >/dev/null 2>&1 || { echo "ERROR: git not found"; exit 1; }
	@command -v gh >/dev/null 2>&1 || { echo "ERROR: gh not found — install from https://cli.github.com/"; exit 1; }
	git config core.hooksPath .githooks
	@echo "  Git hooks installed from .githooks/"
	@if command -v rustup >/dev/null 2>&1; then rustup component add rustfmt clippy; else echo "  WARNING: rustup not found — ensure rustfmt and clippy are available"; fi
	@gh extension list | grep -q 'gh stack' && echo "  ✓ gh stack found" || echo "  - gh stack not found (gh extension install github/gh-stack)"
	@command -v uv >/dev/null 2>&1 && echo "  ✓ uv found" || echo "  - uv not found (required for make docs-serve)"
	@command -v vhs >/dev/null 2>&1 && echo "  ✓ vhs found" || echo "  - vhs not found (required for make demo-gif)"

setup-hooks: ## Install versioned git hooks
	git config core.hooksPath .githooks

format: ## Apply Rust formatting
	cargo fmt --all

lint: ## Check formatting and Clippy
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
tests: ## Run the full suite
	cargo test --workspace --locked --no-fail-fast

preflight: lint tests ## Run the CI-equivalent checks

live-test: ## Run end-to-end tests against real gh and gh stack (needs auth and push access to the fixture repo)
	STACK_STAT_LIVE=1 cargo test -p stack-stat-core --test live_gh --locked -- --test-threads=4

live-fixture-check: ## Verify the live fixture's tags, branches, PR states and labels
	bash scripts/live-fixture.sh check

live-fixture-reset: ## Restore the live fixture branches to their start tags
	bash scripts/live-fixture.sh reset

rustdoc: ## Build rustdoc and open it
	cargo doc --workspace --no-deps --locked
	@echo "Rustdoc built in target/doc; open stack_stat_core/index.html with your platform's browser."

docs-serve: ## Preview the documentation site
	@bash scripts/serve-docs.sh --watch

demo: ## Run the dashboard against the fictional demo stack
	cargo run -q -p stack-stat-tui --example demo

demo-gif: ## Re-record docs/user/assets/demo.gif with vhs
	@command -v vhs >/dev/null 2>&1 || { echo "ERROR: vhs not found — install from https://github.com/charmbracelet/vhs"; exit 1; }
	vhs docs/user/assets/demo.tape

clean: ## Remove Cargo build artifacts
	cargo clean
