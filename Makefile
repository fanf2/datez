.PHONY: it doc

it:
	cargo fmt
	cargo clippy
	cargo test
	cargo doc --no-deps
	cargo run
