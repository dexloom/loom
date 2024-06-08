# Loom project

## Crates

- [actors](./crates/actors) - actors implementation
- [actors-macros](./crates/actors-macros) - macros for actors
- [debug-provider](./crates/debug-provider) - debug api provider for node + anvil
- [defi-abi](./crates/defi-abi) - sol! wrapper for contracts interface
- [defi-actors](./crates/defi-actors) - defi actors crate
- [defi-entities](./crates/defi-entities) - defi entities crate
- [defi-envents](./crates/defi-events) - defi events crate
- [defi-pools](./crates/defi-pools) - defi exchange pools implementation
- [flashbots](./crates/flashbots) - flashbots client
- [multicaller](./crates/multicaller) - multicaller interaction crate
- [topology](./crates/topology) - topology crate
- [types](./crates/types) - defi types crate
- [utils](./crates/utils) - various helpers

## Bins

- [loom](./bin/loom) - backrun bot
- [loom_anvil](./bin/loom_anvil) - testing framework
- [keys](./bin/keys) - keys encryption tool

Telegram chat : https://t.me/dexloom_com

## Starting

### Setting up topology

config.toml example

```toml
[node]
mode = "ws"

# Nodes. 
[clients]
local = { url = "ws://localhost:8545" }
remote = { url = "ws://8.8.8.8:8545" }

[blockchains]
# Ethereum mainnet. chain id = 1
mainnet = { }

# Setup signer with encrypted private key
[signers]
env_signer = { type = "env", bc = "mainnet" }

# Swapstep encoder with address of multicaller deployed
[encoders]
mainnet = { type = "swapstep", address = "0x0000000000000000000000000000000000000000" }

# Preloaders for signers and encoders
[preloaders]
mainnet = { client = "local", bc = "mainnet", encoder = "mainnet", signers = "env_signer" }


[actors]
# Blocks managing actor
[actors.node]
mainnet_node = { client = "local", bc = "mainnet" }

# Subscribe to mempool transactions
[actors.mempool]
mainnet = { client = "local", bc = "mainnet" }
mainnet_remote = { client = "remote", bc = "mainnet" }

# Nonce and balance monitor
[actors.noncebalance]
mainnet = { client = "local", bc = "mainnet" }


# Pool loader : history, new and protocol loaders
[actors.pools]
mainnet = { client = "local", bc = "mainnet", history = true, new = true, protocol = true }

# Price actor 
[actors.price]
mainnet = { client = "local", bc = "mainnet" }

# Broadcaster actor 
[actors.broadcaster]
mainnet = { type = "flashbots", client = "local", bc = "mainnet" }

# Transaction estimators
[actors.estimator]
# Node estimator. Geth is ok for note supporting eth_callBundle method only 
mainnet = { client = "local", bc = "mainnet", type = "geth", encoder = "mainnet" }
# EVM estimator
#mainnet = { type="evm", bc = "mainnet",  encoder = "mainnet"}
```

### Setting private key

please creata defi-entities/private.rs with following context

```rust
pub const KEY_ENCRYPTION_PWD: [u8; 16] = [35, 48, 129, 101, 133, 220, 104, 197, 183, 159, 203, 89, 168, 201, 91, 130];
```

To change key encryption password run

```sh
cargo run --bin keys generate-password  
```

To get encrypted key run:

```sh
cargo run --bin keys encrypt --key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

### Starting loom

```sh
DATA=<ENCRYPTED_PRIVATE_KEY> cargo run --bin loom
```

# GREETINGS

- [Pradigm](https://github.com/paradigmxyz) - Paradigm. All those inspiring products : RETH / REVM / Alloy / Ethers
- [darkforestry](https://github.com/darkforestry/amms-rs) - AMM Crate
- [0xKitsune](https://github.com/0xKitsune) - Uniswap Math crate
- [Onbjerg](https://github.com/onbjerg) - Flashbots crate

# DISCLAMER

THE SOFTWARE IS PROVIDED "AS IS", USE AT YOUR OWN RISK
