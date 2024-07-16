# NodeBench utility

Helps to understand and compare node performance

```
./nodebench ws://node1 ws://node2
```

## Reth WS vx Geth WS

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

## Reth WS vs ExEx vs Geth

### Test 1

```
headers abs first [1, 0, 10] avg delay [251023, 231753, 82487] μs
headers rel first [1, 0, 10] avg delay [260164, 240043, 73346] μs
blocks abs first [0, 1] avg delay [1755415, 0] μs
blocks rel first [0, 1] avg delay [1837707, 0] μs
logs abs first [0, 1, 9] avg delay [174506, 184194, 99545] μs
logs rel first [0, 1, 9] avg delay [182735, 192301, 90424] μs
state abs first [0, 8, 2] avg delay [45224, 28023, 84996] μs
state rel first [0, 7, 3] avg delay [47123, 24944, 86792] μs

txs total in blocks: 1716 received by nodes: 1147 per node [1141, 1146, 1127]  outdated [28, 0, 1]
txs abs first [642, 443, 62] delays avg [7834, 1288, 21241] μs
txs rel first [444, 390, 313] delays avg [5627, 1198, 27634] μs
```

### Test 2

```
headers abs first [0, 0, 11] avg delay [185971, 189554, 0] μs
headers rel first [0, 0, 11] avg delay [194603, 198189, 0] μs
blocks abs first [1, 0, 9] avg delay [161018, 132533, 20363] μs
blocks rel first [1, 0, 9] avg delay [169650, 139441, 11731] μs
logs abs first [1, 0, 9] avg delay [158259, 130098, 12212] μs
logs rel first [1, 0, 9] avg delay [166892, 137006, 3580] μs
state abs first [0, 7, 3] avg delay [42114, 26966, 55220] μs
state rel first [0, 6, 4] avg delay [45091, 27673, 54998] μs

txs total in blocks: 1729 received by nodes: 1250 per node [1244, 1249, 1226]  outdated [7, 0, 1]
txs abs first [726, 448, 75] delays avg [2554, 1733, 22996] μs
txs rel first [546, 344, 359] delays avg [1900, 1533, 30334] μs
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




