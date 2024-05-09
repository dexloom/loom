# Set of actors for Defi

Blockchain actors

- [accounts_monitor](./src/accounts_monitor) - monitors nonce and ETH balance of address list
- [blockhistory](./src/block_history) - handles block updates and reorgs, stores blocks data
- [gas](./src/gas) - gas worker. handles current gas price
- [mempool](./src/mempool) - handles mempool transactions
- [signers](./src/signers) - sign transactions
- [tx_broadcaster](./src/tx_broadcaster) - broadcast transactions
- [node](./src/node) - responsible for collecting information from nodes and passing it forward

Market actors:

- [price](./src/price) - price worker
- [market](./src/market) - works pools and tokens meta data
- [market_state](./src/market_state) - preloads non-deployed contacts to in-memory EVM state
- [health_monitor](./src/health_monitor) - handles state of pools, stuffing txes and in-memory EVM state

Searchers:

- [estimators](./src/estimators) - set of gas estimators for EVM and Geth nodes and local test provider, responsible for
  tips
- [backrun](./src/backrun) - searches for arb opportunities in blocks and mempool transactions
- [mergers](./src/mergers) - paths merger
- [pathencoder](./src/pathencoder) - paths encoders
- 
