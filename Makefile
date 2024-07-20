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
	cargo test --all

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
.PHONY: fmt-check
fmt-check:
	cargo +stable fmt --all --check

# check files format with fmt and clippy
.PHONY: pre-release
pre-release:
	cargo +stable fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings


# format loom
.PHONY: fmt
fmt:
	cargo +stable fmt --all

.PHONY: swap-test
swap-test:
	@echo "Running anvil swap test case: $(FILE)\n"
	@RL=${RL:-info}; \
	RUST_LOG=$(RL) cargo run --package loom_anvil --bin loom_anvil -- --config $(FILE); \
	EXIT_CODE=$$?; \
	if [ $$EXIT_CODE -ne 0 ]; then \
		echo "\n\033[0;31mError: Anvil swap tester exited with code $$EXIT_CODE\033[0m\n"; \
	else \
		echo "\n\033[0;32mAnvil swap test passed successfully.\033[0m"; \
	fi

.PHONY: swap-test-1
swap-test-1: FILE="./bin/loom_anvil/test_18498188.toml"
swap-test-1: swap-test

.PHONY: swap-test-2
swap-test-2: FILE="./bin/loom_anvil/test_18567709.toml"
swap-test-2: swap-test

.PHONY: swap-test-3
swap-test-3: FILE="./bin/loom_anvil/test_19101578.toml"
swap-test-3: swap-test

.PHONY: swap-test-4
swap-test-4:FILE="./bin/loom_anvil/test_19109955.toml"
swap-test-4: swap-test

.PHONY: swap-test-all
swap-test-all: RL=off
swap-test-all:
	@$(MAKE) swap-test-1 RL=$(RL)
	@$(MAKE) swap-test-2 RL=$(RL)
	@$(MAKE) swap-test-3 RL=$(RL)
	@$(MAKE) swap-test-4 RL=$(RL)


