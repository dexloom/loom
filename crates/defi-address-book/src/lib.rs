use alloy_primitives::{address, Address};

#[non_exhaustive]
pub struct Token;

impl Token {
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
pub struct Factory;

impl Factory {
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
pub struct Periphery;

impl Periphery {
    pub const UNISWAP_V2_ROUTER: Address = address!("7a250d5630b4cf539739df2c5dacb4c659f2488d");
    pub const UNISWAP_V3_QUOTER: Address = address!("b27308f9F90D607463bb33eA1BeBb41C27CE5AB6");
    pub const UNISWAP_V3_TICK_LENS: Address = address!("bfd8137f7d1516D3ea5cA83523914859ec47F573");
    pub const PANCAKE_V3_QUOTER: Address = address!("B048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997");
    pub const PANCAKE_V3_TICK_LENS: Address = address!("9a489505a00cE272eAa5e07Dba6491314CaE3796");
    pub const MAVERICK_QUOTER: Address = address!("9980ce3b5570e41324904f46A06cE7B466925E23");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_token() {
        assert_eq!(Token::WETH, address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"));
    }
}
