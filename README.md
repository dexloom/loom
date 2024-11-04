# Loom

<div align="center">

[![CI status](https://github.com/dexloom/loom/actions/workflows/ci.yml/badge.svg?branch=main)][gh-loom]
[![Book status](https://github.com/dexloom/loom/actions/workflows/book.yml/badge.svg?branch=main)][gh-book]
[![Telegram Chat][tg-badge]][tg-url]

| [User Book](https://dexloom.github.io/loom/)
| [Crate Docs](https://dexloom.github.io/loom/docs/) |

[gh-loom]: https://github.com/dexloom/loom/actions/workflows/ci.yml
[gh-book]: https://github.com/dexloom/loom/actions/workflows/book.yml
[tg-badge]: https://img.shields.io/badge/telegram-dexloom_com-2CA5E0?style=plastic&logo=telegram
[tg-url]: https://t.me/dexloom_com

The toolbox for your DeFi strategies:
![Loom components](book/images/loom_components.svg)

</div>

## What is Loom?

Loom is a modular framework designed to streamline the development of automated strategies for decentralized exchanges (DEXs) or other blockchain applications.

## Who is Loom for?

First of all, Loom will not generate any revenue and is challenging for newcomers. The Loom framework is tailored for advanced users with prior experience in blockchain development. It’s specifically designed for developers building trading bots, arbitrage bots, block builders, solvers, or those looking to work with blockchain events.

## How to get started?

See the [Getting started](https://dexloom.github.io/loom/getting_started.html) guide. Have also a look at the [Multicaller](https://github.com/dexloom/multicaller) smart contract repository.


## Crates

- [actors](crates/core/actors) - actors implementation
- [actors-macros](crates/core/actors-macros) - macros for actors
- [debug-provider](crates/node/debug-provider) - debug api provider for node + anvil, HttpCachedTransport
- [defi-abi](crates/defi/abi) - sol! wrapper for contracts interface
- [defi-blockchain](crates/core/blockchain) - loom configuration module
- [defi-entities](crates/types/entities) - defi entities crate
- [defi-events](crates/types/events) - defi events crate
- [defi-pools](crates/defi/pools) - defi exchange pools implementation
- [flashbots](crates/broadcast/flashbots) - flashbots client
- [loom-revm-db](crates/evm/db) - optimized InMemoryDB
- [multicaller](crates/execution/multicaller) - multicaller interaction crate
- [topology](crates/core/topology) - topology crate
- [types](crates/types/blockchain) - blockchain types crate
- [utils](crates/evm/utils) - various helpers

## Bins

- [loom](./bin/loom_backrun) - backrun bot
- [loom_exex](./bin/loom_exex) - backrun bot as ExEx module
- [loom_anvil](./bin/loom_anvil) - anvil testing framework
- [replayer](./bin/replayer) - blocks replayer
- [keys](./bin/keys) - keys encryption tool
- [gasbench](./bin/gasbench) - gas consumption benchmark utility
- [nodebench](./bin/nodebench) - nodes benchmark utility


# GREETINGS

- [Paradigm](https://github.com/paradigmxyz) - Paradigm. All those inspiring products : RETH / REVM / Alloy / Ethers
- [darkforestry](https://github.com/darkforestry/amms-rs) - AMM Crate
- [0xKitsune](https://github.com/0xKitsune) - Uniswap Math crate
- [Onbjerg](https://github.com/onbjerg) - Flashbots crate

# DISCLAMER

THE SOFTWARE IS PROVIDED "AS IS", USE AT YOUR OWN RISK
