use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolInterface;
use eyre::eyre;
use loom_defi_abi::uniswap2::IUniswapV2Pair;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_entities::Pool;

pub struct UniswapV2ProtocolAbiEncoder;

impl UniswapV2ProtocolAbiEncoder {
    #[inline]
    pub fn get_zero_for_one(token_address_from: &Address, token_address_to: &Address) -> bool {
        token_address_from < token_address_to
    }
}

impl ProtocolAbiSwapEncoderTrait for UniswapV2ProtocolAbiEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        _pool: &dyn Pool<LoomDataTypesEthereum>,
        _token_from_address: Address,
        _token_to_address: Address,
        _amount: U256,
        _recipient: Address,
        _payload: Bytes,
    ) -> eyre::Result<Bytes> {
        Err(eyre!("NOT_SUPPORTED"))
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
        let swap_call = if UniswapV2ProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address) {
            IUniswapV2Pair::swapCall { amount0Out: U256::ZERO, amount1Out: amount, to: recipient, data: payload }
        } else {
            IUniswapV2Pair::swapCall { amount0Out: amount, amount1Out: U256::ZERO, to: recipient, data: payload }
        };

        Ok(Bytes::from(IUniswapV2Pair::IUniswapV2PairCalls::swap(swap_call).abi_encode()))
    }

    fn swap_out_amount_offset(&self, _pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if UniswapV2ProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address) {
            Some(0x24)
        } else {
            Some(0x04)
        }
    }

    fn swap_out_amount_return_offset(&self, _pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        if UniswapV2ProtocolAbiEncoder::get_zero_for_one(&token_from_address, &token_to_address) {
            Some(0x20)
        } else {
            Some(0x00)
        }
    }

    fn swap_in_amount_offset(&self, _pool: &dyn Pool, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
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
