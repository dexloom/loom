use alloy_primitives::{address, Address};

#[non_exhaustive]
pub struct TokenAddress;

impl TokenAddress {
    pub const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    pub const USDC: Address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    pub const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    pub const DAI: Address = address!("6b175474e89094c44da98b954eedeac495271d0f");
    pub const WBTC: Address = address!("2260fac5e5542a773aa44fbcfedf7c193bc2c599");
    pub const THREECRV: Address = address!("6c3f90f043a72fa612cbac8115ee7e52bde6e490");
    pub const CRV: Address = address!("d533a949740bb3306d119cc777fa900ba034cd52");
    pub const STETH: Address = address!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");
    pub const WSTETH: Address = address!("7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0");
}

#[non_exhaustive]
pub struct FactoryAddress;

impl FactoryAddress {
    // Uniswap V2 compatible
    pub const UNISWAP_V2: Address = address!("5c69bee701ef814a2b6a3edd4b1652cb9cc5aa6f");
    pub const SUSHISWAP_V2: Address = address!("c0aee478e3658e2610c5f7a4a2e1777ce9e4f2ac");
    pub const NOMISWAP: Address = address!("818339b4e536e707f14980219037c5046b049dd4");
    pub const DOOARSWAP: Address = address!("1e895bfe59e3a5103e8b7da3897d1f2391476f3c");
    pub const SAFESWAP: Address = address!("7f09d4be6bbf4b0ff0c97ca5c486a166198aeaee");
    pub const MINISWAP: Address = address!("2294577031f113df4782b881cf0b140e94209a6f");
    pub const SHIBASWAP: Address = address!("115934131916c8b277dd010ee02de363c09d037c");
    pub const OG_PEPE: Address = address!("52fba58f936833f8b643e881ad308b2e37713a86");

    // Uniswap V3 compatible
    pub const UNISWAP_V3: Address = address!("1f98431c8ad98523631ae4a59f267346ea31f984");
    pub const SUSHISWAP_V3: Address = address!("baceb8ec6b9355dfc0269c18bac9d6e2bdc29c4f");
    pub const PANCAKE_V3: Address = address!("0bfbcf9fa4f9c56b0f40a671ad40e0805a091865");

    // Maverick
    pub const MAVERICK: Address = address!("eb6625d65a0553c9dbc64449e56abfe519bd9c9b");
}

#[non_exhaustive]
pub struct PeripheryAddress;

impl PeripheryAddress {
    pub const UNISWAP_V2_ROUTER: Address = address!("7a250d5630b4cf539739df2c5dacb4c659f2488d");
    pub const UNISWAP_V3_QUOTER: Address = address!("b27308f9f90d607463bb33ea1bebb41c27ce5ab6");
    pub const UNISWAP_V3_TICK_LENS: Address = address!("bfd8137f7d1516d3ea5ca83523914859ec47f573");
    pub const PANCAKE_V3_QUOTER: Address = address!("b048bbc1ee6b733fffcfb9e9cef7375518e25997");
    pub const PANCAKE_V3_TICK_LENS: Address = address!("9a489505a00ce272eaa5e07dba6491314cae3796");
    pub const MAVERICK_QUOTER: Address = address!("9980ce3b5570e41324904f46a06ce7b466925e23");
}

#[non_exhaustive]
pub struct UniswapV2PoolAddress;

impl UniswapV2PoolAddress {
    pub const LUSD_WETH: Address = address!("f20ef17b889b437c151eb5ba15a47bfc62bff469");
    pub const WETH_USDT: Address = address!("0d4a11d5eeaac28ec3f61d100daf4d40471f1852");
}

#[non_exhaustive]
pub struct UniswapV3PoolAddress;

impl UniswapV3PoolAddress {
    pub const USDC_USDT_100: Address = address!("3416cf6c708da44db2624d63ea0aaef7113527c6");
    pub const USDC_WETH_500: Address = address!("88e6a0c2ddd26feeb64f039a2c41296fcb3f5640");
    pub const USDC_WETH_3000: Address = address!("8ad599c3a0ff1de082011efddc58f1908eb6e6d8");
    pub const WBTC_USDT_3000: Address = address!("9db9e0e53058c89e5b94e29621a205198648425b");
    pub const WETH_USDT_3000: Address = address!("4e68ccd3e89f51c3074ca5072bbac773960dfa36");
}

#[non_exhaustive]
pub struct PancakeV2PoolAddress;

impl PancakeV2PoolAddress {
    pub const WETH_USDT: Address = address!("17c1ae82d99379240059940093762c5e4539aba5");
}

#[non_exhaustive]
pub struct PancakeV3PoolAddress;

impl PancakeV3PoolAddress {
    pub const USDC_USDT_100: Address = address!("04c8577958ccc170eb3d2cca76f9d51bc6e42d8f");
}

#[non_exhaustive]
pub struct CurvePoolAddress;

impl CurvePoolAddress {
    pub const DAI_USDC_USDT: Address = address!("bebc44782c7db0a1a60cb6fe97d0b483032ff1c7");
    pub const USDT_BTC_ETH: Address = address!("d51a44d3fae010294c616388b506acda1bfaae46");
    pub const ETH_BTC_USD: Address = address!("7f86bf177dd4f3494b841a37e810a34dd56c829b");
    pub const FRXETH_WETH: Address = address!("9c3b46c0ceb5b9e304fcd6d88fc50f7dd24b31bc");
    pub const ETH: Address = address!("a1f8a6807c402e4a15ef4eba36528a3fed24e577");
}

#[non_exhaustive]
pub struct CurveMetapoolAddress;

impl CurveMetapoolAddress {
    pub const LUSD: Address = address!("ed279fdd11ca84beef15af5d39bb4d4bee23f0ca");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_token() {
        assert_eq!(TokenAddress::WETH, address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"));
    }
}
