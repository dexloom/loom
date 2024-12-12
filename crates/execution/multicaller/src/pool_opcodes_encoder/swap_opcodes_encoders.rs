use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::{
    CurveSwapOpcodesEncoder, SwapOpcodesEncoderTrait, UniswapV2SwapOpcodesEncoder, UniswapV3SwapOpcodesEncoder,
};
use alloy_primitives::Address;
use eyre::OptionExt;
use loom_types_blockchain::MulticallerCalls;
use loom_types_entities::{Pool, PoolClass, SwapAmountType};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProtocolSwapOpcodesEncoderV2 {
    pool_classes: HashMap<PoolClass, Arc<dyn SwapOpcodesEncoderTrait>>,
}

impl Default for ProtocolSwapOpcodesEncoderV2 {
    fn default() -> Self {
        let mut pool_classes: HashMap<PoolClass, Arc<dyn SwapOpcodesEncoderTrait>> = HashMap::new();

        let uni2_opcodes_encoder = Arc::new(UniswapV2SwapOpcodesEncoder {});
        let uni3_opcodes_encoder = Arc::new(UniswapV3SwapOpcodesEncoder {});
        let curve_opcodes_encoder = Arc::new(CurveSwapOpcodesEncoder {});

        pool_classes.insert(PoolClass::UniswapV2, uni2_opcodes_encoder.clone());
        pool_classes.insert(PoolClass::Maverick, uni3_opcodes_encoder.clone());
        pool_classes.insert(PoolClass::UniswapV3, uni3_opcodes_encoder.clone());
        pool_classes.insert(PoolClass::PancakeV3, uni3_opcodes_encoder.clone());
        pool_classes.insert(PoolClass::Curve, curve_opcodes_encoder.clone());

        Self { pool_classes }
    }
}

impl SwapOpcodesEncoderTrait for ProtocolSwapOpcodesEncoderV2 {
    fn encode_swap_in_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_in: SwapAmountType,
        cur_pool: &dyn Pool,
        next_pool: Option<&dyn Pool>,
        multicaller: Address,
    ) -> eyre::Result<()> {
        let opcodes_encoder = self.pool_classes.get(&cur_pool.get_class()).ok_or_eyre("OPCODES_ENCODER_NOT_FOUND")?;
        opcodes_encoder.encode_swap_in_amount_provided(
            swap_opcodes,
            abi_encoder,
            token_from_address,
            token_to_address,
            amount_in,
            cur_pool,
            next_pool,
            multicaller,
        )
    }
}