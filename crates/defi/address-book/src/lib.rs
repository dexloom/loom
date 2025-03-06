use alloy_primitives::{address, Address};

#[non_exhaustive]
pub struct TokenAddressEth;

impl TokenAddressEth {
    pub const ETH_NATIVE: Address = Address::ZERO;
    pub const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    pub const USDC: Address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    pub const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    pub const DAI: Address = address!("6b175474e89094c44da98b954eedeac495271d0f");
    pub const WBTC: Address = address!("2260fac5e5542a773aa44fbcfedf7c193bc2c599");
    pub const THREECRV: Address = address!("6c3f90f043a72fa612cbac8115ee7e52bde6e490");
    pub const CRV: Address = address!("d533a949740bb3306d119cc777fa900ba034cd52");
    pub const STETH: Address = address!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");
    pub const WSTETH: Address = address!("7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0");
    pub const LUSD: Address = address!("5f98805a4e8be255a32880fdec7f6728c6568ba0");

    pub fn is_weth(&address: &Address) -> bool {
        address.eq(&Self::WETH)
    }
    pub fn is_eth(&address: &Address) -> bool {
        address.eq(&Self::ETH_NATIVE)
    }
}

#[non_exhaustive]
pub struct TokenAddressArbitrum;

impl TokenAddressArbitrum {
    pub const ETH_NATIVE: Address = Address::ZERO;
    pub const WETH: Address = address!("82aF49447D8a07e3bd95BD0d56f35241523fBab1");
    pub const USDC: Address = address!("af88d065e77c8cC2239327C5EDb3A432268e5831");
    pub const WBTC: Address = address!("2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f");
    pub const USDT: Address = address!("Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
    pub const DAI: Address = address!("DA10009cBd5D07dd0CeCc66161FC93D7c9000da1");
    pub const CRV: Address = address!("11cDb42B0EB46D95f990BeDD4695A6e3fA034978");

    pub fn is_weth(&address: &Address) -> bool {
        address.eq(&Self::WETH)
    }
    pub fn is_eth(&address: &Address) -> bool {
        address.eq(&Self::ETH_NATIVE)
    }
}

#[non_exhaustive]
pub struct TokenAddressBase;
impl TokenAddressBase {
    pub const ETH_NATIVE: Address = Address::ZERO;
    pub const WETH: Address = address!("4200000000000000000000000000000000000006");
    pub const USDC: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
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
    pub const ANTFARM: Address = address!("E48AEE124F9933661d4DD3Eb265fA9e153e32CBe");
    pub const INTEGRAL: Address = address!("C480b33eE5229DE3FbDFAD1D2DCD3F3BAD0C56c6");

    // Uniswap V3 compatible
    pub const UNISWAP_V3: Address = address!("1f98431c8ad98523631ae4a59f267346ea31f984");
    pub const SUSHISWAP_V3: Address = address!("baceb8ec6b9355dfc0269c18bac9d6e2bdc29c4f");
    pub const PANCAKE_V3: Address = address!("0bfbcf9fa4f9c56b0f40a671ad40e0805a091865");

    // Maverick
    pub const MAVERICK: Address = address!("eb6625d65a0553c9dbc64449e56abfe519bd9c9b");

    pub const MAVERICK_V2: Address = address!("0A7e848Aca42d879EF06507Fca0E7b33A0a63c1e");

    pub const UNISWAP_V4_POOL_MANAGER_ADDRESS: Address = address!("000000000004444c5dc75cB358380D2e3dE08A90");
}

#[non_exhaustive]
pub struct PeripheryAddress;

impl PeripheryAddress {
    pub const UNISWAP_PERMIT_2_ADDRESS: Address = address!("000000000022D473030F116dDEE9F6B43aC78BA3");
    pub const UNISWAP_V2_ROUTER: Address = address!("7a250d5630b4cf539739df2c5dacb4c659f2488d");
    pub const UNISWAP_V3_QUOTER_V2: Address = address!("61ffe014ba17989e743c5f6cb21bf9697530b21e");
    pub const UNISWAP_V3_TICK_LENS: Address = address!("bfd8137f7d1516d3ea5ca83523914859ec47f573");
    pub const PANCAKE_V3_QUOTER: Address = address!("b048bbc1ee6b733fffcfb9e9cef7375518e25997");
    pub const PANCAKE_V3_TICK_LENS: Address = address!("9a489505a00ce272eaa5e07dba6491314cae3796");
    pub const MAVERICK_QUOTER: Address = address!("9980ce3b5570e41324904f46a06ce7b466925e23");
    pub const UNISWAP_V4_QUOTER: Address = address!("52f0e24d1c21c8a0cb1e5a5dd6198556bd9e1203");
    pub const UNISWAPV4_STATE_VIEW_ADDRESS: Address = address!("7fFE42C4a5DEeA5b0feC41C94C136Cf115597227");
    pub const MAVERICK_V2_QUOTER: Address = address!("b40AfdB85a07f37aE217E7D6462e609900dD8D7A");
    pub const MAVERICK_V2_TICK_LENS: Address = address!("6A9EB38DE5D349Fe751E0aDb4c0D9D391f94cc8D");
}

#[non_exhaustive]
pub struct UniswapV2PoolAddress;

impl UniswapV2PoolAddress {
    pub const LUSD_WETH: Address = address!("f20ef17b889b437c151eb5ba15a47bfc62bff469");
    pub const WETH_USDT: Address = address!("0d4a11d5eeaac28ec3f61d100daf4d40471f1852");
    pub const USDC_WETH: Address = address!("0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");
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
    pub const STETH_ETH: Address = address!("dc24316b9ae028f1497c275eb9192a3ea0f67022");
    pub const STETH_WETH: Address = address!("828b154032950c8ff7cf8085d841723db2696056");
}

#[non_exhaustive]
pub struct CurveMetapoolAddress;

impl CurveMetapoolAddress {
    pub const LUSD: Address = address!("ed279fdd11ca84beef15af5d39bb4d4bee23f0ca");
}

#[non_exhaustive]
pub struct MaverickV2PoolAddress;

impl MaverickV2PoolAddress {
    pub const USDC_USDT: Address = address!("31373595F40Ea48a7aAb6CBCB0d377C6066E2dCA");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_token() {
        assert_eq!(TokenAddressEth::WETH, address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"));
    }
}
