use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use alloy_primitives::{Address, Bytes, U256};
use eyre::OptionExt;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_entities::Pool;

pub struct CurveProtocolAbiEncoder;

impl ProtocolAbiSwapEncoderTrait for CurveProtocolAbiEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        pool: &dyn Pool<LoomDataTypesEthereum>,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes> {
        pool.get_abi_encoder().ok_or_eyre("NO_POOL_ENCODER")?.encode_swap_in_amount_provided(
            token_from_address,
            token_to_address,
            amount,
            recipient,
            payload,
        )
    }

    fn encode_swap_out_amount_provided(
        &self,
        pool: &dyn Pool,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes> {
        pool.get_abi_encoder().ok_or_eyre("NO_POOL_ENCODER")?.encode_swap_out_amount_provided(
            token_from_address,
            token_to_address,
            amount,
            recipient,
            payload,
        )
    }

    fn swap_in_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        Some(0x44)
    }

    fn swap_out_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }

    fn swap_out_amount_return_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }

    fn swap_in_amount_return_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }

    fn swap_out_amount_return_script(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }

    fn swap_in_amount_return_script(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }
}
