# Define the RUSTFLAGS to treat warnings as errors
RELEASEFLAGS = -D warnings

# Target to run all tests
.PHONY: build
build:
	cargo build --all

release:
	export RELEASEFLAGS | $(CARGO) build --release

# Target to run all tests
.PHONY: test
test:
	cargo test

# Target to run all benchmarks
.PHONY: clean
clean:
	cargo clean

# Target to run all benchmarks
.PHONY: bench
bench:
	cargo bench

# Target to run cargo clippy
.PHONY: clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# check files format fmt
.PHONY: clippy
fmt-check:
	cargo +stable fmt --all --check

# check files format with fmt and clippy
.PHONY: clippy
pre-release:
	cargo +stable fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings


# format loom
.PHONY: clippy
fmt:
	cargo +stable fmt --all