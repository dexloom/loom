use alloy::primitives::{Address, U256};
use loom_defi_address_book::TokenAddressEth;
use std::ops::{Add, Mul};

pub struct NWETH {}

impl NWETH {
    const NWETH_EXP_U128: u128 = 10u128.pow(18);

    const NWETH_EXP: f64 = 10u64.pow(18) as f64;
    const GWEI_EXP_U128: u128 = 10u128.pow(9);
    const GWEI_EXP: f64 = 10u64.pow(9) as f64;
    const WEI_EXP_U128: u128 = 10u128.pow(18);
    const WEI_EXP: f64 = 10u64.pow(18) as f64;

    pub const ADDRESS: Address = TokenAddressEth::WETH;
    pub const NATIVE_ADDRESS: Address = Address::ZERO;

    #[inline]
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

    #[inline]
    pub fn to_float_gwei(value: u128) -> f64 {
        let div = value / Self::GWEI_EXP_U128;
        let rem = value % Self::GWEI_EXP_U128;

        div as f64 + ((rem as f64) / Self::GWEI_EXP)
    }

    #[inline]
    pub fn to_float_wei(value: u128) -> f64 {
        let div = value / Self::WEI_EXP_U128;
        let rem = value % Self::WEI_EXP_U128;

        div as f64 + ((rem as f64) / Self::WEI_EXP)
    }

    #[inline]
    pub fn from_float(value: f64) -> U256 {
        let multiplier = U256::from(value as i64);
        let modulus = U256::from(((value - value.round()) * 10_i64.pow(18) as f64) as u64);
        multiplier.mul(U256::from(10).pow(U256::from(18))).add(modulus)
    }

    #[inline]
    pub fn get_exp() -> U256 {
        U256::from(Self::NWETH_EXP_U128)
    }

    #[inline]
    pub fn address() -> Address {
        Self::ADDRESS
    }
}
