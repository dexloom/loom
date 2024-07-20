# ARB CASE TESTER WITH ANVIL

Each test case is described in config file that contains preloading
information and expected calculation results.

```toml
# WETH GROK AND WBTC ARB CASE WITH  NEW WBTC CRV POOL
# 0x67857131Ae32f72739a2c8d0bd0e812793D8BB24 UNI3 CRV-WBTC Pool created
[modules]
signer = false
price = true


[settings]
block = 18498188
coinbase = "0x1dd35b4da6534230ff53048f7477f17f7f4e7a70"
multicaller = "0x3dd35b4da6534230ff53048f7477f17f7f4e7a70"
skip_default = false

[pools]
weth_wbtc_uni3 = { address = "0xCBCdF9626bC03E24f779434178A73a0B4bad62eD", class = "uniswap3" }
weth_wbtc_uni2 = { address = "0xbb2b8038a1640196fbe3e38816f3e67cba72d940", class = "uniswap2" }
weth_crv_uni3 = { address = "0x919Fa96e88d67499339577Fa202345436bcDaf79", class = "uniswap3" }
weth_crv_uni2 = { address = "0x3da1313ae46132a397d90d95b1424a9a7e3e0fce", class = "uniswap2" }

[txs]

tx_1 = { hash = "0x26177953373b45fa2abac4dee9634c7db65a9e0aaf64b99c7095f51d229f24b7", send = "mempool" }
tx_2 = { hash = "0x1114432ef38437dde663aeb868f553e0ec0ca973120e472687957223efeda331", send = "mempool" }

[tokens]
weth = { address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", symbol = "WETH", decimals = 18, basic = true, middle = false }
wbtc = { address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", symbol = "WBTC", decimals = 8, basic = true, middle = false }

[results]
swaps_encoded = 14
swaps_ok = 11
best_profit_eth = 181.37
```

Run current available tests

```shell
make swap-test-all
```

Run specific

```shell
make swap-test FILE=<PATH_TO_TEST_CONFIG>
```

