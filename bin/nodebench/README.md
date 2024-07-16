# NodeBench utility

Helps to understand and compare node performance

```
./nodebench ws://node1 ws://node2
```

## Reth vx Geth

Stat result for [Reth/Geth], started on Reth instance

```
headers abs first [0, 11] avg delay [258388, 0] μs
headers rel first [0, 11] avg delay [266549, 0] μs
blocks abs first [0, 10] avg delay [212087, 0] μs
blocks rel first [0, 10] avg delay [220249, 0] μs
logs abs first [0, 10] avg delay [208387, 0] μs
logs rel first [0, 10] avg delay [216548, 0] μs
state abs first [4, 6] avg delay [135129, 59128] μs
state rel first [4, 6] avg delay [143290, 50966] μs

txs total in blocks: 1757 received by nodes: 1123 per node [1117, 1101]  outdated [11, 0]
txs abs first [1038, 85] delays avg [35150, 20861] μs
txs rel first [797, 326] delays avg [9165, 27169] μs
```

## Reth WS vs ExEx

```
headers abs first [11, 0] avg delay [0, 3856] μs
blocks abs first [5, 6] avg delay [1349, 1195] μs
logs abs first [8, 3] avg delay [2275, 827] μs
state abs first [0, 10] avg delay [55310, 0] μs

txs total in blocks: 1846 received by nodes: 1136 per node [1125, 1136]  outdated [21, 0]
txs abs first [766, 370] delays avg [4782, 166] μs
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




