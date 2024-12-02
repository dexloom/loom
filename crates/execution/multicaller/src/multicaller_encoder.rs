use alloy_primitives::{Address, Bytes};
use eyre::{eyre, OptionExt, Result};
use tracing::error;

use crate::abi_encoders::{ProtocolABIEncoderV2, ProtocolAbiSwapEncoderTrait};
use crate::SwapStepEncoder;
use loom_types_blockchain::MulticallerCalls;
use loom_types_entities::Swap;

pub trait MulticallerEncoder {
    fn encode_calls(&self, calls: MulticallerCalls) -> Result<(Address, Bytes)>;
    fn add_internal_calls(&self, opcodes: MulticallerCalls, inside_opcodes: MulticallerCalls) -> Result<MulticallerCalls>;
    fn make_calls(&self, swap: &Swap) -> Result<MulticallerCalls>;
}

#[derive(Clone)]
pub struct MulticallerSwapEncoder<E: ProtocolAbiSwapEncoderTrait = ProtocolABIEncoderV2> {
    pub multicaller_address: Address,
    pub swap_step_encoder: SwapStepEncoder<E>,
}

impl<E: ProtocolAbiSwapEncoderTrait> MulticallerSwapEncoder<E> {
    pub fn new(multicaller_address: Address, abi_encoder: E) -> Self {
        Self { multicaller_address, swap_step_encoder: SwapStepEncoder::new(multicaller_address, abi_encoder) }
    }

    pub fn get_contract_address(&self) -> Address {
        self.multicaller_address
    }
}

impl<E: ProtocolAbiSwapEncoderTrait> MulticallerEncoder for MulticallerSwapEncoder<E> {
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
