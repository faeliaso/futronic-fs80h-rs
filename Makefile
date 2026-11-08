.PHONY: check test lint validate

check:
	cargo fmt --check

test:
	cargo test --all-targets

lint:
	cargo clippy --all-targets -- -D warnings

validate: check lint test
