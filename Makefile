# Define the RUSTFLAGS to treat warnings as errors
RELEASEFLAGS = -D warnings -C target-cpu=native

#### Targets ####
## All targets
# Target to build the project
.PHONY: build
build:
	cargo build --all

# Build release
.PHONY: release
release:
	export RELEASEFLAGS | cargo build --release

# Build optimized release
.PHONY: maxperf
maxperf:
	export RELEASEFLAGS | cargo build --profile maxperf

## Exex gRPC node
# Target to build the Exex gRPC node
.PHONY: build-exex-node
build-exex-node:
	cargo build --bin exex-grpc-node

# Build release for Exex gRPC node
.PHONY: release-exex-node
release-exex-node:
	export RELEASEFLAGS | cargo build --bin exex-grpc-node --release

# Build optimized release of Exex gRPC node
.PHONY: maxperf-exex-node
maxperf-exex-node:
	export RELEASEFLAGS | cargo build --bin exex-grpc-node --profile maxperf

## Development commands
# Target to run all tests
.PHONY: test
test:
	cargo test --all --all-features

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

# format loom
.PHONY: fmt
fmt:
	cargo +stable fmt --all

# check files format fmt
.PHONY: fmt-check
fmt-check:
	cargo +stable fmt --all --check

# format toml
.PHONY: taplo
taplo:
	taplo format

# check files format with taplo
.PHONY: taplo-check
taplo-check:
	taplo format --check

# check licences
.PHONY: deny-check
deny-check:
	cargo deny --all-features check

# check files format with fmt and clippy
.PHONY: pre-release
pre-release:
	cargo +stable fmt --all --check
	cargo clippy --all-targets --all-features -- -D warnings

# replayer test
.PHONY: replayer
replayer:
	@echo "Running Replayer test case: $(FILE)\n"
	@RL=${RL:-info}; \
	RUST_LOG=$(RL) cargo run --package replayer --bin replayer --; \
	EXIT_CODE=$$?; \
	if [ $$EXIT_CODE -ne 0 ]; then \
		echo "\n\033[0;31mError: Replayer tester exited with code $$EXIT_CODE\033[0m\n"; \
	else \
		echo "\n\033[0;32mReplayer test passed successfully.\033[0m"; \
	fi

# swap tests with loom_anvil
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


