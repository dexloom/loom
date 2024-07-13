.PHONY: build run test clean

build:
	cargo build --all

release:
	cargo build --release

test:
	cargo test --all --all-features

clean:
	cargo clean

fmt:
	cargo +stable fmt --all