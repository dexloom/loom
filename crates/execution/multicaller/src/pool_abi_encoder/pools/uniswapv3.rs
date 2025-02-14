use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use alloy_primitives::{Address, Bytes, I256, U160, U256};
use alloy_sol_types::SolInterface;
use lazy_static::lazy_static;
use loom_defi_abi::uniswap3::IUniswapV3Pool;
use loom_types_entities::Pool;
use std::ops::Sub;

lazy_static! {
    static ref LOWER_LIMIT: U160 = U160::from(4295128740u64);
    static ref UPPER_LIMIT: U160 = U160::from_str_radix("1461446703485210103287273052203988822378723970341", 10).unwrap();
}

pub struct UniswapV3ProtocolAbiEncoder;
impl UniswapV3ProtocolAbiEncoder {
    pub fn get_zero_for_one(token_address_from: &Address, token_address_to: &Address) -> bool {
        token_address_from < token_address_to
    }

    pub fn get_price_limit(token_address_from: &Address, token_address_to: &Address) -> U160 {
        if token_address_from < token_address_to {
            *LOWER_LIMIT
        } else {
            *UPPER_LIMIT
        }
    }
}

impl ProtocolAbiSwapEncoderTrait for UniswapV3ProtocolAbiEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        _pool: &dyn Pool,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes> {
        let zero_for_one = UniswapV3ProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = UniswapV3ProtocolAbiEncoder::get_price_limit(&token_from_address, &token_to_address);
        let swap_call = IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            amountSpecified: I256::from_raw(amount),
            sqrtPriceLimitX96: sqrt_price_limit_x96,
            data: payload,
        };

        Ok(Bytes::from(IUniswapV3Pool::IUniswapV3PoolCalls::swap(swap_call).abi_encode()))
    }

    fn encode_swap_out_amount_provided(
        &self,
        _pool: &dyn Pool,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes> {
        let zero_for_one = UniswapV3ProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = UniswapV3ProtocolAbiEncoder::get_price_limit(&token_from_address, &token_to_address);
        let swap_call = IUniswapV3Pool::swapCall {
            recipient,
            zeroForOne: zero_for_one,
            amountSpecified: I256::ZERO.sub(I256::from_raw(amount)),
            sqrtPriceLimitX96: sqrt_price_limit_x96,
            data: payload,
        };

        Ok(Bytes::from(IUniswapV3Pool::IUniswapV3PoolCalls::swap(swap_call).abi_encode()))
    }

    fn swap_in_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn swap_out_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn swap_out_amount_return_offset(&self, _pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if Self::get_zero_for_one(&token_from_address, &token_to_address) {
            Some(0x20)
        } else {
            Some(0x0)
        }
    }

    fn swap_in_amount_return_offset(&self, _pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if Self::get_zero_for_one(&token_from_address, &token_to_address) {
            Some(0x20)
        } else {
            Some(0x0)
        }
    }

    fn swap_in_amount_return_script(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        Some(Bytes::from(vec![0x8, 0x2A, 0x00]))
    }
    fn swap_out_amount_return_script(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }
}
