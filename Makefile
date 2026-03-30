# Makefile for selection-capture

.PHONY: help build test check fmt clippy clean docs run-examples release install-tools setup-release windows-beta-smoke linux-alpha-smoke bench-regression agent-docs agent-docs-check

# Default target
.DEFAULT_GOAL := help

help: ## Display this help message
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the project
	cargo build

test: ## Run tests
	cargo test --verbose

windows-beta-smoke: ## Run the Windows beta smoke tests
	cargo test --features windows-beta --test windows_smoke --verbose

linux-alpha-smoke: ## Run the Linux alpha smoke tests
	cargo test --features linux-alpha --test linux_smoke --verbose

check: ## Check code without building
	cargo check --all-targets

fmt: ## Format code
	cargo fmt --all

fmt-check: ## Check code formatting
	cargo fmt --all -- --check

clippy: ## Run Clippy linter
	cargo clippy --all-targets --all-features -- -D warnings

lint: fmt-check clippy ## Run all linting checks

clean: ## Clean build artifacts
	cargo clean

docs: ## Build documentation
	cargo doc --no-deps --document-private-items --open

docs-check: ## Check documentation
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items

agent-docs: ## Generate AI-agent docs from Cargo metadata
	python3 scripts/generate_agent_docs.py

agent-docs-check: ## Ensure AGENTS.md and llms.txt are in sync
	python3 scripts/check_agent_docs_sync.py

audit: ## Run security audit
	cargo install cargo-audit
	cargo audit

update-deps: ## Update dependencies
	cargo update

outdated: ## Check for outdated dependencies
	cargo install cargo-outdated
	cargo outdated

release: lint test ## Prepare for release
	@echo "Building release..."
	cargo build --release
	@echo "Release build complete!"

install-tools: ## Install release automation tools
	./scripts/install-tools.sh

setup-release: install-tools ## Setup complete release environment
	./scripts/setup-release.sh

release-patch: lint test ## Release patch version (0.x.0 -> 0.x.1)
	cargo release patch --execute

release-minor: lint test ## Release minor version (0.x.0 -> 0.(x+1).0)
	cargo release minor --execute

release-major: lint test ## Release major version (x.0.0 -> (x+1).0.0)
	cargo release major --execute

release-dry-run: ## Dry-run release (test what would happen)
	cargo release patch --test

changelog: ## Generate changelog from git commits
	git cliff -o CHANGELOG.md

changelog-preview: ## Preview unreleased changelog changes
	git cliff --unreleased --preview

version-bump: ## Bump version manually (usage: make version-bump VER=0.2.0)
	cargo set-version $(VER)

publish: lint test ## Publish to crates.io
	cargo publish

publish-dry-run: ## Test publish without actually publishing
	cargo publish --dry-run

dev: ## Run in development mode with hot reload (if using cargo-watch)
	cargo install cargo-watch
	cargo watch -x 'check'

example-%: ## Run a specific example (e.g., make example-simple)
	cargo run --example $*

bench: ## Run benchmarks
	cargo bench

bench-regression: ## Run benchmark regression guardrails
	cargo bench --bench capture_latency -- --noplot
	python3 scripts/check_bench_regression.py

coverage: ## Generate code coverage report
	cargo install cargo-tarpaulin
	cargo tarpaulin --out Html

ci: fmt-check clippy agent-docs-check test ## Run CI checks locally
