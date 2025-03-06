use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolInterface;
use lazy_static::lazy_static;
use loom_defi_abi::maverick::IMaverickPool;
use loom_defi_abi::maverick::IMaverickPool::IMaverickPoolCalls;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_entities::Pool;

lazy_static! {
    static ref LOWER_LIMIT: U256 = U256::from(4295128740u64);
    static ref UPPER_LIMIT: U256 = U256::from_str_radix("1461446703485210103287273052203988822378723970341", 10).unwrap();
}

pub struct MaverickProtocolAbiEncoder;

impl MaverickProtocolAbiEncoder {
    pub fn get_zero_for_one(token_address_from: &Address, token_address_to: &Address) -> bool {
        token_address_from < token_address_to
    }

    pub fn get_price_limit(token_address_from: &Address, token_address_to: &Address) -> U256 {
        if *token_address_from < *token_address_to {
            *LOWER_LIMIT
        } else {
            *UPPER_LIMIT
        }
    }
}

impl ProtocolAbiSwapEncoderTrait for MaverickProtocolAbiEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        _pool: &dyn Pool<LoomDataTypesEthereum>,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes> {
        let token_a_in = MaverickProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address);

        let swap_call = IMaverickPool::swapCall {
            recipient,
            amount,
            tokenAIn: token_a_in,
            exactOutput: false,
            sqrtPriceLimit: U256::ZERO,
            data: payload,
        };

        Ok(Bytes::from(IMaverickPoolCalls::swap(swap_call).abi_encode()))
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
        let token_a_in = MaverickProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address);
        let sqrt_price_limit_x96 = MaverickProtocolAbiEncoder::get_price_limit(&token_from_address, &token_to_address);

        let swap_call = IMaverickPool::swapCall {
            recipient,
            amount,
            tokenAIn: token_a_in,
            exactOutput: true,
            sqrtPriceLimit: sqrt_price_limit_x96,
            data: payload,
        };

        Ok(Bytes::from(IMaverickPoolCalls::swap(swap_call).abi_encode()))
    }

    fn swap_in_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x24)
    }

    fn swap_out_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x24)
    }

    fn swap_out_amount_return_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x20)
    }

    fn swap_in_amount_return_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x20)
    }

    fn swap_out_amount_return_script(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }

    fn swap_in_amount_return_script(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }
}
