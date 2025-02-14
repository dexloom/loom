use crate::pool_abi_encoder::pools::{
    CurveProtocolAbiEncoder, MaverickProtocolAbiEncoder, PancakeV3ProtocolAbiEncoder, UniswapV2ProtocolAbiEncoder,
    UniswapV3ProtocolAbiEncoder,
};
use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use alloy_primitives::{Address, Bytes, U256};
use eyre::OptionExt;
use loom_types_entities::{Pool, PoolClass};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProtocolABIEncoderV2 {
    pool_classes: HashMap<PoolClass, Arc<dyn ProtocolAbiSwapEncoderTrait>>,
}

impl Default for ProtocolABIEncoderV2 {
    fn default() -> Self {
        let pool_classes: HashMap<PoolClass, Arc<dyn ProtocolAbiSwapEncoderTrait>> = [
            (PoolClass::UniswapV3, Arc::new(UniswapV3ProtocolAbiEncoder) as Arc<dyn ProtocolAbiSwapEncoderTrait>),
            (PoolClass::UniswapV2, Arc::new(UniswapV2ProtocolAbiEncoder) as Arc<dyn ProtocolAbiSwapEncoderTrait>),
            (PoolClass::Maverick, Arc::new(MaverickProtocolAbiEncoder) as Arc<dyn ProtocolAbiSwapEncoderTrait>),
            (PoolClass::PancakeV3, Arc::new(PancakeV3ProtocolAbiEncoder) as Arc<dyn ProtocolAbiSwapEncoderTrait>),
            (PoolClass::Curve, Arc::new(CurveProtocolAbiEncoder) as Arc<dyn ProtocolAbiSwapEncoderTrait>),
        ]
        .into_iter()
        .collect();

        Self { pool_classes }
    }
}

impl ProtocolABIEncoderV2 {}

impl ProtocolAbiSwapEncoderTrait for ProtocolABIEncoderV2 {
    fn encode_swap_in_amount_provided(
        &self,
        pool: &dyn Pool,
        token_from_address: Address,
        token_to_address: Address,
        amount: U256,
        recipient: Address,
        payload: Bytes,
    ) -> eyre::Result<Bytes> {
        self.pool_classes.get(&pool.get_class()).ok_or_eyre("CLASS_NOT_SUPPORTED")?.encode_swap_in_amount_provided(
            pool,
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
        self.pool_classes.get(&pool.get_class()).ok_or_eyre("CLASS_NOT_SUPPORTED")?.encode_swap_out_amount_provided(
            pool,
            token_from_address,
            token_to_address,
            amount,
            recipient,
            payload,
        )
    }

    fn swap_in_amount_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        self.pool_classes
            .get(&pool.get_class())
            .and_then(|encoder| encoder.swap_in_amount_offset(pool, token_from_address, token_to_address))
    }

    fn swap_out_amount_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        self.pool_classes
            .get(&pool.get_class())
            .and_then(|encoder| encoder.swap_out_amount_offset(pool, token_from_address, token_to_address))
    }

    fn swap_in_amount_return_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        self.pool_classes
            .get(&pool.get_class())
            .and_then(|encoder| encoder.swap_in_amount_return_offset(pool, token_from_address, token_to_address))
    }
    fn swap_out_amount_return_offset(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<u32> {
        self.pool_classes
            .get(&pool.get_class())
            .and_then(|encoder| encoder.swap_out_amount_return_offset(pool, token_from_address, token_to_address))
    }

    fn swap_in_amount_return_script(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<Bytes> {
        self.pool_classes
            .get(&pool.get_class())
            .and_then(|encoder| encoder.swap_in_amount_return_script(pool, token_from_address, token_to_address))
    }
    fn swap_out_amount_return_script(&self, pool: &dyn Pool, token_from_address: Address, token_to_address: Address) -> Option<Bytes> {
        self.pool_classes
            .get(&pool.get_class())
            .and_then(|encoder| encoder.swap_out_amount_return_script(pool, token_from_address, token_to_address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_defi_pools::UniswapV3Pool;
    use loom_types_entities::PreswapRequirement;

    #[test]
    fn test_default() {
        let abi_encoder_v2 = ProtocolABIEncoderV2::default();
        assert_eq!(abi_encoder_v2.pool_classes.len(), 5);
    }

    #[test]
    fn test_preswap_requirement() {
        let uni3 = UniswapV3Pool::new(Address::random());

        let pr = uni3.preswap_requirement();

        assert_eq!(pr, PreswapRequirement::Callback)
    }
}
