use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Div, Mul, Neg};
use std::string::ToString;
use std::sync::Arc;
use std::sync::RwLock;

use alloy_primitives::utils::Unit;
use alloy_primitives::{I256, U256};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};

const ONE_ETHER: U256 = Unit::ETHER.wei_const();

#[derive(Clone, Debug, Default)]
pub struct Token<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    address: LDT::Address,
    basic: bool,
    middle: bool,
    decimals: u8,
    name: Option<String>,
    symbol: Option<String>,
    eth_price: Arc<RwLock<Option<U256>>>,
}

pub type TokenWrapper<LDT> = Arc<Token<LDT>>;

impl<LDT: LoomDataTypes> Hash for Token<LDT> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state)
    }
}

impl<LDT: LoomDataTypes> PartialEq for Token<LDT> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.address == other.get_address()
    }
}

impl<LDT: LoomDataTypes> Eq for Token<LDT> {}

impl<LDT: LoomDataTypes> Ord for Token<LDT> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.address.cmp(&other.get_address())
    }
}

impl<LDT: LoomDataTypes> PartialOrd for Token<LDT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<LDT: LoomDataTypes> Token<LDT> {
    #[inline]
    pub fn new(address: LDT::Address) -> Token<LDT> {
        Self::new_with_data(address, None, None, Some(18), false, false)
    }

    #[inline]
    pub fn zero() -> Token<LDT> {
        Self::new_with_data(LDT::Address::default(), None, None, Some(18), false, false)
    }

    pub fn new_with_data(
        address: LDT::Address,
        symbol: Option<String>,
        name: Option<String>,
        decimals: Option<u8>,
        basic: bool,
        middle: bool,
    ) -> Token<LDT> {
        Token { address, symbol, name, decimals: decimals.unwrap_or(18), basic, middle, eth_price: Arc::new(RwLock::new(None)) }
    }

    #[inline]
    pub fn get_symbol(&self) -> String {
        self.symbol.clone().unwrap_or(self.address.to_string())
    }

    #[inline]
    pub fn get_name(&self) -> String {
        self.name.clone().unwrap_or(self.address.to_string())
    }

    #[inline]
    pub fn get_decimals(&self) -> u8 {
        self.decimals
    }

    #[inline]
    pub fn get_exp(&self) -> U256 {
        if self.decimals == 18 {
            ONE_ETHER
        } else {
            U256::from(10).pow(U256::from(self.decimals))
        }
    }

    #[inline]
    pub fn get_address(&self) -> LDT::Address {
        self.address
    }

    #[inline]
    pub fn is_basic(&self) -> bool {
        self.basic
    }

    #[inline]
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
        if self.decimals == 0 {
            0f64
        } else {
            let divider = self.get_exp();
            let ret = value.div_rem(divider);

            let div = u64::try_from(ret.0);
            let rem = u64::try_from(ret.1);

            if div.is_err() || rem.is_err() {
                0f64
            } else {
                div.unwrap_or_default() as f64 + ((rem.unwrap_or_default() as f64) / (10u64.pow(self.decimals as u32) as f64))
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
        let modulus = U256::from(((value - value.round()) * (10 ^ self.decimals as i64) as f64) as u64);
        multiplier.mul(U256::from(10).pow(U256::from(self.decimals))).add(modulus)
    }

    #[inline]
    pub fn is_weth(&self) -> bool {
        self.address.eq(&LDT::WETH)
    }

    pub fn get_eth_price(&self) -> Option<U256> {
        if self.is_weth() {
            Some(ONE_ETHER)
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
        self.get_eth_price().map(|x| value.mul(ONE_ETHER).div(x))
    }

    pub fn calc_token_value_from_eth(&self, eth_value: U256) -> Option<U256> {
        let x = self.get_eth_price();
        x.map(|x| eth_value.mul(x).div(ONE_ETHER))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use loom_defi_address_book::TokenAddressEth;

    #[test]
    fn test_to_float() {
        let weth_token =
            Token::<LoomDataTypesEthereum>::new_with_data(TokenAddressEth::WETH, Some("WETH".to_string()), None, Some(18), true, false);
        let usdc_token =
            Token::<LoomDataTypesEthereum>::new_with_data(TokenAddressEth::USDC, Some("USDC".to_string()), None, Some(6), false, false);
        let usdt_token =
            Token::<LoomDataTypesEthereum>::new_with_data(TokenAddressEth::USDT, Some("USDT".to_string()), None, Some(6), false, false);
        let dai_token =
            Token::<LoomDataTypesEthereum>::new_with_data(TokenAddressEth::DAI, Some("DAI".to_string()), None, Some(18), false, false);
        let wbtc_token =
            Token::<LoomDataTypesEthereum>::new_with_data(TokenAddressEth::WBTC, Some("WBTC".to_string()), None, Some(8), false, false);

        let one_ether = U256::from(10).pow(U256::from(15));

        println!("{}", weth_token.to_float(one_ether));
    }
}
