use alloy_primitives::{Address, Bytes};
use eyre::{eyre, OptionExt, Result};
use std::sync::Arc;
use tracing::error;

use crate::pool_abi_encoder::ProtocolABIEncoderV2;
use crate::pool_opcodes_encoder::ProtocolSwapOpcodesEncoderV2;
use crate::{SwapLineEncoder, SwapStepEncoder, DEFAULT_VIRTUAL_ADDRESS};
use loom_types_blockchain::MulticallerCalls;
use loom_types_entities::Swap;

pub trait MulticallerEncoder {
    fn encode_calls(&self, calls: MulticallerCalls) -> Result<(Address, Bytes)>;
    fn add_internal_calls(&self, opcodes: MulticallerCalls, inside_opcodes: MulticallerCalls) -> Result<MulticallerCalls>;
    fn make_calls(&self, swap: &Swap) -> Result<MulticallerCalls>;
}

#[derive(Clone)]
pub struct MulticallerSwapEncoder {
    pub multicaller_address: Address,
    pub swap_step_encoder: SwapStepEncoder,
}

impl MulticallerSwapEncoder {
    pub fn new(multicaller_address: Address, swap_step_encoder: SwapStepEncoder) -> Self {
        Self { multicaller_address, swap_step_encoder }
    }

    pub fn default_with_address(multicaller_address: Address) -> Self {
        let abi_encoder = ProtocolABIEncoderV2::default();
        let opcodes_encoder = ProtocolSwapOpcodesEncoderV2::default();

        let swap_line_encoder = SwapLineEncoder::new(multicaller_address, Arc::new(abi_encoder), Arc::new(opcodes_encoder));

        let swap_step_encoder = SwapStepEncoder::new(multicaller_address, swap_line_encoder);

        Self { multicaller_address, swap_step_encoder }
    }

    pub fn get_contract_address(&self) -> Address {
        self.multicaller_address
    }
}

impl MulticallerEncoder for MulticallerSwapEncoder {
    fn encode_calls(&self, calls: MulticallerCalls) -> Result<(Address, Bytes)> {
        self.swap_step_encoder.to_call_data(&calls)
    }

    fn add_internal_calls(&self, opcodes: MulticallerCalls, inside_opcodes: MulticallerCalls) -> Result<MulticallerCalls> {
        self.swap_step_encoder.encode_do_calls(opcodes, inside_opcodes)
    }

    fn make_calls(&self, swap: &Swap) -> Result<MulticallerCalls> {
        match swap {
            Swap::BackrunSwapLine(swap_line) => {
                let (swap_step_0, swap_step_1) = swap_line.to_swap_steps(self.multicaller_address).ok_or_eyre("SWAP_TYPE_NOT_COVERED")?;
                self.swap_step_encoder.encode_swap_steps(&swap_step_0, &swap_step_1)
            }
            Swap::BackrunSwapSteps((swap_step_0, swap_step_1)) => self.swap_step_encoder.encode_swap_steps(swap_step_0, swap_step_1),
            Swap::Multiple(swap_vec) => {
                if swap_vec.len() == 1 {
                    self.make_calls(&swap_vec[0])
                } else {
                    let mut multicaller_calls = MulticallerCalls::new();
                    for swap in swap_vec {
                        multicaller_calls = self.add_internal_calls(multicaller_calls, self.make_calls(swap)?)?;
                    }
                    Ok(multicaller_calls)
                }
            }
            _ => {
                error!("Swap type not supported");
                Err(eyre!("SWAP_TYPE_NOT_SUPPORTED"))
            }
        }
    }
}

impl Default for MulticallerSwapEncoder {
    fn default() -> Self {
        MulticallerSwapEncoder::default_with_address(DEFAULT_VIRTUAL_ADDRESS)
    }
}
