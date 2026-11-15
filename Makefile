# agent-avatar — common developer tasks. Run `make` (or `make help`) to list them.
# Prereqs: Rust toolchain (see docs/DEVELOPMENT.md).

.DEFAULT_GOAL := help
.PHONY: help server serve run test check leakscan release-draft release fmt lint clean

help: ## List available targets
	@grep -E '^[a-z-]+:.*##' $(MAKEFILE_LIST) | sed 's/:.*## /\t/' \
		| awk -F'\t' '{printf "  make %-13s %s\n", $$1, $$2}'

server: ## Build the release server binary
	cargo build --release -p agent-avatar-server

serve: ## Run the server on 127.0.0.1:8070 (AVATAR_ADDR overrides)
	cargo run -p agent-avatar-server

run: serve ## Alias for serve (the page has no build step)

test: ## Run the test suites
	cargo test --workspace

check: ## Pre-commit gate: format check, clippy, tests, leak scan
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	$(MAKE) --no-print-directory leakscan

release-draft: ## Draft the next release's notes at the top of CHANGELOG.md (git-cliff); edit them, then make release
	python3 -I scripts/release.py draft

release: check ## Gate, scan the drafted notes, commit them and tag the release (pushes nothing)
	python3 -I scripts/release.py cut

leakscan: ## Scan all history for personal data (shape rules, plus a denylist from LEAKSCAN_DENYLIST_FILE)
	python3 -I -m unittest discover -s scripts -p 'test_*.py'
	python3 -I scripts/leakscan.py --all

fmt: ## Format all Rust code
	cargo fmt

lint: ## Clippy across the workspace, warnings as errors
	cargo clippy --workspace --all-targets -- -D warnings

clean: ## Remove build output
	cargo clean
