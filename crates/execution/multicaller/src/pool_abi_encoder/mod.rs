use alloy_primitives::{Address, Bytes, U256};
use loom_types_entities::Pool;

pub use abi_encoder::*;
mod abi_encoder;

mod pools;

pub trait ProtocolAbiSwapEncoderTrait: Send + Sync + 'static {
    fn encode_swap_in_amount_provided(
        &self,
        pool: &dyn Pool,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes>;

    fn encode_swap_out_amount_provided(
        &self,
        pool: &dyn Pool,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes>;

    fn swap_in_amount_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32>;

    fn swap_out_amount_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32>;

    fn swap_out_amount_return_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32>;

    fn swap_in_amount_return_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32>;

    fn swap_out_amount_return_script(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<Bytes>;

    fn swap_in_amount_return_script(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<Bytes>;
}
