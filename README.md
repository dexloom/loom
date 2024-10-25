# Loom

<div align="center">

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

## Who is Loom for?

First of all, Loom in its current state will not generate revenue and is challenging for newcomers. The Loom framework is tailored for advanced users with prior experience in blockchain development. It’s specifically designed for developers building trading bots, arbitrage bots, block builders, solvers, or those looking to work with blockchain event.

## Getting starting

See the [Getting started](https://dexloom.github.io/loom/getting_started.html) guide.


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


# GREETINGS

- [Paradigm](https://github.com/paradigmxyz) - Paradigm. All those inspiring products : RETH / REVM / Alloy / Ethers
- [darkforestry](https://github.com/darkforestry/amms-rs) - AMM Crate
- [0xKitsune](https://github.com/0xKitsune) - Uniswap Math crate
- [Onbjerg](https://github.com/onbjerg) - Flashbots crate

# DISCLAMER

THE SOFTWARE IS PROVIDED "AS IS", USE AT YOUR OWN RISK
