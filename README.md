# Loom project

<div style="text-align: center;">

[![CI status](https://github.com/dexloom/loom/workflows/Loom/badge.svg)][gh-loom]
[![Book status](https://github.com/dexloom/loom/workflows/Book/badge.svg)][gh-book]
[![Telegram Chat][tg-badge]][tg-url]

| [User Book](https://dexloom.github.io/loom/)
| [Crate Docs](https://dexloom.github.io/loom/docs/) |

[gh-loom]: https://github.com/dexloom/loom/actions/workflows/ci.yml
[gh-book]: https://github.com/dexloom/loom/actions/workflows/book.yml
[tg-badge]: https://img.shields.io/badge/telegram-dexloom_com-2CA5E0?style=plastic&logo=telegram
[tg-url]: https://t.me/dexloom_com

</div>

## What is Loom?

Loom is a modular framework designed to streamline the development of automated strategies for decentralized exchanges (DEXs) or other blockchain applications.

## Crates

- [actors](./crates/actors) - actors implementation
- [actors-macros](./crates/actors-macros) - macros for actors
- [debug-provider](./crates/debug-provider) - debug api provider for node + anvil, HttpCachedTransport
- [defi-abi](./crates/defi-abi) - sol! wrapper for contracts interface
- [defi-actors](./crates/defi-actors) - defi actors crate
- [defi-blockchain](./crates/defi-blockchain) - loom configuration module
- [defi-entities](./crates/defi-entities) - defi entities crate
- [defi-events](./crates/defi-events) - defi events crate
- [defi-pools](./crates/defi-pools) - defi exchange pools implementation
- [flashbots](./crates/flashbots) - flashbots client
- [loom-revm-db](./crates/loom-revm-db) - optimized InMemoryDB
- [multicaller](./crates/multicaller) - multicaller interaction crate
- [topology](./crates/topology) - topology crate
- [types](./crates/types) - defi types crate
- [utils](./crates/utils) - various helpers

## Bins

- [loom](./bin/loom_backrun) - backrun bot
- [loom_exex](./bin/loom_exex) - backrun bot as ExEx module
- [loom_anvil](./bin/loom_anvil) - anvil testing framework
- [replayer](./bin/replayer) - blocks replayer
- [keys](./bin/keys) - keys encryption tool
- [gasbench](./bin/gasbench) - gas consumption benchmark utility
- [nodebench](./bin/nodebench) - nodes benchmark utility


## Starting

### Setting up topology

Copy `config-example.toml` to `config.toml` and configure according to your setup.

### Updating private key encryption password

Private key encryption password is individual secret key that is generated automatically but can be replaced

It is located in ./crates/defi-entities/private.rs and looks like

```rust
pub const KEY_ENCRYPTION_PWD: [u8; 16] = [35, 48, 129, 101, 133, 220, 104, 197, 183, 159, 203, 89, 168, 201, 91, 130];
```

To change key encryption password run and replace content of KEY_ENCRYPTION_PWD

```sh
cargo run --bin keys generate-password  
```

To get encrypted key run:

```sh
cargo run --bin keys encrypt --key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

### Setup database
Install postgresql and create database and user.

Create user and db:
```shell
su - postgres
createuser loom
createdb loom
```

Run `psql` and update user and privileges:
```psql
alter user loom with encrypted password 'loom';
grant all privileges on database loom to loom;
create schema loom;
grant usage on schema loom to loom;
grant create on schema loom to loom;
\q
```

### Starting loom

```sh
DATA=<ENCRYPTED_PRIVATE_KEY> cargo run --bin loom
```

## Makefile

Makefile is shipped with following important commands:

- build - builds all binaries
- fmt - formats loom with rustfmt
- pre-release - check code with rustfmt and clippy
- clippy - check code with clippy

## Testing

Testing Loom requires two environment variables pointing at archive node:

- MAINNET_WS - websocket url of archive node
- MAINNET_HTTP - http url of archive node

To run tests:

```shell
make test
```

# GREETINGS

- [Paradigm](https://github.com/paradigmxyz) - Paradigm. All those inspiring products : RETH / REVM / Alloy / Ethers
- [darkforestry](https://github.com/darkforestry/amms-rs) - AMM Crate
- [0xKitsune](https://github.com/0xKitsune) - Uniswap Math crate
- [Onbjerg](https://github.com/onbjerg) - Flashbots crate

# DISCLAMER

THE SOFTWARE IS PROVIDED "AS IS", USE AT YOUR OWN RISK
