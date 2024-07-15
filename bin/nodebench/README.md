# NodeBench utility

Helps to understand and compare node performance

```
./nodebench ws://node1 ws://node2
```

## Reth vx Geth

Stat result for [Reth/Geth], started on Reth instance

```
headers abs first [0, 10] avg delay [361909, 0] μs
headers rel first [0, 10] avg delay [370423, 0] μs
blocks abs first [0, 10] avg delay [328226, 0] μs
blocks rel first [0, 10] avg delay [336740, 0] μs
logs abs first [0, 10] avg delay [335986, 0] μs
logs rel first [0, 10] avg delay [344501, 0] μs
state abs first [2, 8] avg delay [259292, 110104] μs
state rel first [2, 8] avg delay [267806, 101589] μs

txs total in blocks: 1580 received by nodes: 1130 per node [1123, 1096]  outdated [13, 0]
txs abs first [593, 537] delays avg ms [28010, 16774]
txs rel first [306, 824] delays avg ms [18254, 32507]
```

blocks abs first - counter of blocks received by node before other nodes
avg ms - average delay in milliseconds
blocks rel first - counter of blocks received by node before other nodes corrected by ping time

txs total - total transactions in observed blocks,
received by nodes - number of block transactions those went through node's mempool
total - number of transactions covered by all mempools
outdated - transactions those are recevied with the large delay, not couunted

txs abs first - txes receved from the node before others
delays avg - averate delay when transaction doesn't come first in milliseconds

txs rel first - txes receved from the node before others corrected by ping time
delays avg - averate delay when transaction doesn't come first in milliseconds corrected by ping time.

## Reth WS vs ExEx

```
headers abs first [10, 0] avg delay [0, 3706] μs
headers rel first [10, 0] avg delay [0, 3703] μs
blocks abs first [7, 3] avg delay [681, 1967] μs
blocks rel first [7, 3] avg delay [683, 1965] μs
logs abs first [6, 4] avg delay [678, 1896] μs
logs rel first [6, 4] avg delay [681, 1893] μs
state abs first [0, 10] avg delay [56646, 0] μs
state rel first [0, 10] avg delay [56649, 0] μs

txs total in blocks: 1832 received by nodes: 1362 per node [1291, 1362]  outdated [43, 0]
txs abs first [933, 429] delays avg ms [1439, 259]
txs rel first [927, 435] delays avg ms [1419, 261]
```

