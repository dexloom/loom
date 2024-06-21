# Gasbench is a gas estimation tool

Make snapshot of gas used for various possible paths.

```sh
cargo run --package gasbench --bin gasbench -- -s first_stat.json
```

Compare current gas consumption with snapshot

```sh
cargo run --package gasbench --bin gasbench -- first_stat.json
```

```
-199 : [UniswapV2, UniswapV3, UniswapV3] ["WETH", "USDT", "USDC", "WETH"] 288174 - 288373 
0 : [UniswapV2, UniswapV3, UniswapV3] ["WETH", "USDT", "USDC", "WETH"] 289213 - 289213 
10000 : [UniswapV3, UniswapV3, UniswapV2] ["WETH", "USDC", "USDT", "WETH"] 256070 - 246070 
```

Snapshot example.

```json
[
  [
    {
      "pool_types": [
        "UniswapV2",
        "UniswapV3",
        "UniswapV3"
      ],
      "token_symbols": [
        "WETH",
        "USDT",
        "USDC",
        "WETH"
      ],
      "pools": [
        "0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852",
        "0x3416cf6c708da44db2624d63ea0aaef7113527c6",
        "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8"
      ],
      "tokens": [
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
      ]
    },
    288373
  ]
]
```

TODO: Add more paths.
