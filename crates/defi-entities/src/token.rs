use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Div, Mul, Neg};
use std::sync::Arc;
use std::sync::RwLock;

use alloy_primitives::{Address, I256, U256};
use lazy_static::lazy_static;

lazy_static! {
    static ref WETH_ADDRESS: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
    static ref ONE_ETHER: U256 = U256::from(10).pow(U256::from(18));
}

#[derive(Clone, Debug, Default)]
pub struct Token {
    address: Address,
    basic: bool,
    middle: bool,
    decimals: Option<i32>,
    name: Option<String>,
    symbol: Option<String>,
    eth_price: Arc<RwLock<Option<U256>>>,
}

pub type TokenWrapper = Arc<Token>;

/*
impl Default for Token {
    fn default() -> Self {
        Token{
            address: Address::zero(),
            basic : false,
            middle : false,
            decimals : None,
            name : None,
            symbol: None,
            eth_price : None),
        }
    }
}
*/

impl Hash for Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state)
    }
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.get_address()
    }
}

impl Eq for Token {}

impl Ord for Token {
    fn cmp(&self, other: &Self) -> Ordering {
        self.address.cmp(&other.get_address())
    }
}

impl PartialOrd for Token {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Token {
    pub fn new(address: Address) -> Token {
        Token { address, decimals: Some(18), ..Token::default() }
    }

    pub fn new_with_data(
        address: Address,
        symbol: Option<String>,
        name: Option<String>,
        decimals: Option<i32>,
        basic: bool,
        middle: bool,
    ) -> Token {
        Token { address, symbol, name, decimals, basic, middle, ..Default::default() }
    }

    pub fn get_eth_price(&self) -> Option<U256> {
        if self.is_weth() {
            Some(*ONE_ETHER)
        } else {
            match self.eth_price.read() {
                Ok(x) => *x,
                _ => None,
            }
        }
    }

    pub fn set_eth_price(&self, price: Option<U256>) {
        if let Ok(mut x) = self.eth_price.write() {
            *x = price;
        }
    }

    pub fn calc_eth_value(&self, value: U256) -> Option<U256> {
        self.get_eth_price().map(|x| value.mul(*ONE_ETHER).div(x))
    }

    pub fn calc_token_value_from_eth(&self, eth_value: U256) -> Option<U256> {
        let x = self.get_eth_price();
        x.map(|x| eth_value.mul(x).div(*ONE_ETHER))
    }

    pub fn get_symbol(&self) -> String {
        self.symbol.clone().unwrap_or(self.address.to_string())
    }

    pub fn get_name(&self) -> String {
        self.name.clone().unwrap_or(self.address.to_string())
    }

    pub fn get_decimals(&self) -> Option<i32> {
        self.decimals
    }

    pub fn get_exp(&self) -> U256 {
        let decimals = self.decimals.unwrap_or(18);
        if decimals == 18 {
            *ONE_ETHER
        } else {
            U256::from(10).pow(U256::from(self.decimals.unwrap_or(18)))
        }
    }

    pub fn get_address(&self) -> Address {
        self.address
    }

    pub fn is_basic(&self) -> bool {
        self.basic
    }

    pub fn is_middle(&self) -> bool {
        self.middle
    }

    pub fn set_basic(&mut self) -> &mut Self {
        self.basic = true;
        self
    }

    pub fn set_middle(&mut self) -> &mut Self {
        self.middle = true;
        self
    }

    pub fn to_float(&self, value: U256) -> f64 {
        let decimals = self.decimals.unwrap_or(18);
        if decimals == 0 {
            0f64
        } else {
            let divider = self.get_exp();
            let ret = value.div_rem(divider);

            let div = u64::try_from(ret.0);
            let rem = u64::try_from(ret.1);

            if div.is_err() || rem.is_err() {
                0f64
            } else {
                div.unwrap_or_default() as f64 + ((rem.unwrap_or_default() as f64) / ((10u64.pow(decimals as u32)) as f64))
            }
        }
    }

    pub fn to_float_sign(&self, value: I256) -> f64 {
        let r: U256 = if value.is_positive() { value.into_raw() } else { value.neg().into_raw() };
        let f = self.to_float(r);
        if value.is_positive() {
            f
        } else {
            -f
        }
    }

    pub fn from_float(&self, value: f64) -> U256 {
        let multiplier = U256::from(value as i64);
        let modulus = U256::from(((value - value.round()) * (10 ^ self.decimals.unwrap() as i64) as f64) as u64);
        multiplier.mul(U256::from(10).pow(U256::from(self.decimals.unwrap()))).add(modulus)
    }

    pub fn is_weth(&self) -> bool {
        self.address == *WETH_ADDRESS
    }
}

pub struct NWETH {}

impl NWETH {
    const NWETH_EXP: f64 = 10u64.pow(18) as f64;
    const GWEI_EXP_U128: u128 = 10u128.pow(9);
    const GWEI_EXP: f64 = 10u64.pow(9) as f64;
    const WEI_EXP_U128: u128 = 10u128.pow(18);
    const WEI_EXP: f64 = 10u64.pow(18) as f64;

    pub fn to_float(value: U256) -> f64 {
        let divider = U256::from(Self::NWETH_EXP);

        let ret = value.div_rem(divider);

        let div = u64::try_from(ret.0);
        let rem = u64::try_from(ret.1);

        if div.is_err() || rem.is_err() {
            0f64
        } else {
            div.unwrap_or_default() as f64 + ((rem.unwrap_or_default() as f64) / Self::NWETH_EXP)
        }
    }

    pub fn to_float_gwei(value: u128) -> f64 {
        let div = value / Self::GWEI_EXP_U128;
        let rem = value % Self::GWEI_EXP_U128;

        div as f64 + ((rem as f64) / Self::GWEI_EXP)
    }

    pub fn to_float_wei(value: u128) -> f64 {
        let div = value / Self::WEI_EXP_U128;
        let rem = value % Self::WEI_EXP_U128;

        div as f64 + ((rem as f64) / Self::WEI_EXP)
    }

    pub fn from_float(value: f64) -> U256 {
        let multiplier = U256::from(value as i64);
        let modulus = U256::from(((value - value.round()) * 10_i64.pow(18) as f64) as u64);
        multiplier.mul(U256::from(10).pow(U256::from(18))).add(modulus)
    }

    pub fn get_exp() -> U256 {
        *ONE_ETHER
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_to_float() {
        let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let usdc_address: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap();
        let usdt_address: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap();
        let dai_address: Address = "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap();
        let wbtc_address: Address = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".parse().unwrap();

        let weth_token = Token::new_with_data(weth_address, Some("WETH".to_string()), None, Some(18), true, false);
        let usdc_token = Token::new_with_data(usdc_address, Some("USDC".to_string()), None, Some(6), false, false);
        let usdt_token = Token::new_with_data(usdt_address, Some("USDT".to_string()), None, Some(6), false, false);
        let dai_token = Token::new_with_data(dai_address, Some("DAI".to_string()), None, Some(18), false, false);
        let wbtc_token = Token::new_with_data(wbtc_address, Some("WBTC".to_string()), None, Some(8), false, false);

        let one_ether = U256::from(10).pow(U256::from(15));

        println!("{}", weth_token.to_float(one_ether));
    }
}
