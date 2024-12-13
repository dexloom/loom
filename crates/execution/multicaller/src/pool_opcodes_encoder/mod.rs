use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
pub use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use alloy_primitives::Address;
pub use curve::CurveSwapOpcodesEncoder;
use eyre::Result;
use loom_types_blockchain::MulticallerCalls;
use loom_types_entities::{Pool, SwapAmountType};
pub use steth::StEthSwapEncoder;
pub use swap_opcodes_encoders::ProtocolSwapOpcodesEncoderV2;
pub use uniswap2::UniswapV2SwapOpcodesEncoder;
pub use uniswap3::UniswapV3SwapOpcodesEncoder;
pub use wsteth::WstEthSwapEncoder;

mod curve;
mod steth;
mod uniswap2;
mod uniswap3;
mod wsteth;

mod swap_opcodes_encoders;

pub trait SwapOpcodesEncoderTrait: Send + Sync + 'static {
    #[allow(clippy::too_many_arguments)]
    fn encode_swap_in_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_in: SwapAmountType,
        cur_pool: &dyn Pool,
        next_pool: Option<&dyn Pool>,
        payload: MulticallerOpcodesPayload,
        multicaller_address: Address,
    ) -> Result<()>;

    #[allow(clippy::too_many_arguments)]
    fn encode_swap_out_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_out: SwapAmountType,
        cur_pool: &dyn Pool,
        next_pool: Option<&dyn Pool>,
        payload: MulticallerOpcodesPayload,
        multicaller_address: Address,
    ) -> Result<()>;
}
