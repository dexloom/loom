# Contributing to Loom
We welcome contributions to Loom! Please read the following guidelines before submitting a pull request.

## Run those command before submitting a PR
Before open a PR, please make sure that all tests are passing and the code is properly formatted.

### Run all tests
```bash
make test
make swap-test-all
```

### Format code
```bash
make clippy
make fmt
make taplo
```

## Optional: Install pre-commit hooks
See https://pre-commit.com for a detailed guide on how to install pre-commit hooks.
Then run in the root of the repository:
```bash
pre-commit install
```

## Install tools
To install the tools required to run the tests and format the code, run:
```bash
cargo install taplo-cli --locked
```