# NodeBench utility 

Helps to understand and compare node performance 

```
./nodebench ws://node1 ws://node2
```


Stat result for [Reth/Geth], started on Reth instance

```
Hello, nodebench!
Ping time 0 : PT0.000338227S
Ping time 1 : PT0.008380913S
Warmign up 1 : 20155409 block received 0x3e57dcd2379aa6dc65ce52ef388a5d54a20842c2c869be6e68e816b0ee84f871
Warmign up 0 : 20155409 block received 
...
0x0c60a1ad5f5dc51d71e38196659d74a4ae52efacc74bb1dbc3a1405a8e44ef45 2024-06-23 15:54:00.698031960 +00:00
0 : 20155412 block received 0x0c60a1ad5f5dc51d71e38196659d74a4ae52efacc74bb1dbc3a1405a8e44ef45 2024-06-23 15:54:00.993692266 +00:00
...


blocks abs first [0, 10] avg ms [389, 0]
blocks rel first [0, 10] avg ms [397, 0]
txs total : 1688 received by nodes  [1121, 1122] total 1138, outdated [9, 0]
txs abs first [1028, 110] delays avg ms [53, 18]
txs rel first [729, 409] delays avg ms [14, 25]

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
delays avg  - averate delay when transaction doesn't come first in milliseconds corrected by ping time. 



