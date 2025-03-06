use alloy_primitives::{Address, Bytes, U256};
use eyre::Result;
use lazy_static::lazy_static;
use tracing::trace;

use crate::opcodes_encoder::{OpcodesEncoder, OpcodesEncoderV2};
use crate::SwapLineEncoder;
use loom_defi_abi::AbiEncoderHelper;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{SwapAmountType, SwapStep};

lazy_static! {
    static ref BALANCER_VAULT_ADDRESS: Address = "0xBA12222222228d8Ba445958a75a0704d566BF2C8".parse().unwrap();
}

#[derive(Clone)]
pub struct SwapStepEncoder {
    pub multicaller_address: Address,
    pub swap_line_encoder: SwapLineEncoder,
}

impl SwapStepEncoder {
    pub fn new(multicaller_address: Address, swap_line_encoder: SwapLineEncoder) -> Self {
        Self { multicaller_address, swap_line_encoder }
    }

    pub fn default_with_address(multicaller_address: Address) -> Self {
        let swap_line_encoder = SwapLineEncoder::default_with_address(multicaller_address);
        Self { multicaller_address, swap_line_encoder }
    }

    pub fn get_contract_address(&self) -> Address {
        self.multicaller_address
    }

    pub fn encode_do_calls(&self, opcodes: MulticallerCalls, inside_opcodes: MulticallerCalls) -> Result<MulticallerCalls> {
        let mut opcodes = opcodes;
        let call_bytes = OpcodesEncoderV2::pack_do_calls(&inside_opcodes)?;
        opcodes.add(MulticallerCall::new_call(self.multicaller_address, &call_bytes));
        Ok(opcodes)
    }

    pub fn encode_tips(
        &self,
        swap_opcodes: MulticallerCalls,
        token_address: Address,
        min_balance: U256,
        tips: U256,
        funds_to: Address,
    ) -> Result<MulticallerCalls> {
        self.swap_line_encoder.encode_tips(swap_opcodes, token_address, min_balance, tips, funds_to)
    }

    pub fn encode_balancer_flash_loan(&self, steps: Vec<SwapStep<LoomDataTypesEthereum>>) -> Result<MulticallerCalls> {
        let flash_funds_to = self.multicaller_address;

        let mut swap_opcodes = MulticallerCalls::new();

        let first_swap = &steps[0];

        let mut steps = steps.clone();

        let token = first_swap.first_token().unwrap();
        let in_amount = first_swap.get_in_amount().unwrap();

        for (swap_idx, swap) in steps.iter_mut().enumerate() {
            if swap_idx > 0 {
                swap.get_mut_swap_line_by_index(swap.len() - 1).amount_in = SwapAmountType::Balance(flash_funds_to);
            }

            if swap.swap_line_vec().len() == 1 {
                swap_opcodes.merge(self.swap_line_encoder.encode_swap_line_in_amount(swap.swap_line_vec().first().unwrap(), None)?);
            } else {
                for swap_path in swap.swap_line_vec().iter() {
                    let opcodes = self.swap_line_encoder.encode_swap_line_in_amount(swap_path, None)?;
                    let call_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes)?;
                    swap_opcodes.add(MulticallerCall::new_call(self.multicaller_address, &call_bytes));
                }
            }
        }

        let inside_call_bytes = OpcodesEncoderV2::pack_do_calls_data(&swap_opcodes)?;

        let mut flash_opcodes = MulticallerCalls::new();

        let flash_call_data =
            AbiEncoderHelper::encode_balancer_flashloan(token.get_address(), in_amount, inside_call_bytes, self.multicaller_address);

        flash_opcodes.add(MulticallerCall::new_call(*BALANCER_VAULT_ADDRESS, &flash_call_data));

        Ok(flash_opcodes)
    }

    pub fn encode_in_amount(
        &self,
        flash_step: SwapStep<LoomDataTypesEthereum>,
        swap_step: SwapStep<LoomDataTypesEthereum>,
    ) -> Result<MulticallerCalls> {
        let mut swap_step = swap_step;

        //let flash_funds_to = self.multicaller_address;

        if flash_step.len() > 1 || swap_step.len() > 1 {
            swap_step.get_mut_swap_line_by_index(swap_step.len() - 1).amount_in = SwapAmountType::Balance(self.multicaller_address);
        }

        let mut swap_opcodes = MulticallerCalls::new();

        if swap_step.swap_line_vec().len() == 1 {
            trace!("swap.swap_line_vec().len() == 1");
            swap_opcodes.merge(self.swap_line_encoder.encode_swap_line_in_amount(swap_step.swap_line_vec().first().unwrap(), None)?);
        } else {
            trace!("swap.swap_line_vec().len() != 1");
            for swap_path in swap_step.swap_line_vec().iter() {
                let opcodes = self.swap_line_encoder.encode_swap_line_in_amount(swap_path, None)?;
                let call_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes)?;
                swap_opcodes.add(MulticallerCall::new_call(self.multicaller_address, &call_bytes));

                //swap_opcodes.merge( self.swap_path_encoder.encode_swap_in_amount(swap_path, flash_funds_to, self.multicaller)?);
                //let pop_opcode = Opcode::new_calculation_call(&Bytes::from(vec![0x8,0x8,0x11,0]));
                //swap_opcodes.add(pop_opcode);
            }
        }

        let flash_funds_to = swap_step.get_first_pool();

        let mut flash_swaps = flash_step.swap_line_vec().clone();
        flash_swaps.reverse();

        for flash_swap_path in flash_swaps.iter() {
            swap_opcodes = self.swap_line_encoder.encode_flash_swap_line_in_amount(flash_swap_path, swap_opcodes, flash_funds_to)?;
        }

        Ok(swap_opcodes)
    }

    pub fn encode_out_amount(
        &self,
        swap_step: SwapStep<LoomDataTypesEthereum>,
        flash_step: SwapStep<LoomDataTypesEthereum>,
    ) -> Result<MulticallerCalls> {
        let mut swap_opcodes = MulticallerCalls::new();

        if swap_step.swap_line_vec().len() == 1 {
            swap_opcodes.merge(
                self.swap_line_encoder
                    .encode_swap_line_in_amount(swap_step.swap_line_vec().first().unwrap(), flash_step.get_first_pool())?,
            );
        } else {
            for swap_path in swap_step.swap_line_vec().iter() {
                let opcodes = self.swap_line_encoder.encode_swap_line_in_amount(swap_path, None)?;
                let call_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes)?;
                swap_opcodes.add(MulticallerCall::new_call(self.multicaller_address, &call_bytes));

                //swap_opcodes.merge( self.swap_path_encoder.encode_swap_in_amount(swap_path, flash_funds_to, self.multicaller)?);
                //let pop_opcode = Opcode::new_calculation_call(&Bytes::from(vec![0x8,0x8,0x11,0]));
                //swap_opcodes.add(pop_opcode);
            }
        }

        let mut flash_swaps = flash_step.swap_line_vec().clone();
        flash_swaps.reverse();

        for flash_swap_path in flash_swaps.iter() {
            swap_opcodes = self.swap_line_encoder.encode_flash_swap_line_out_amount(flash_swap_path, swap_opcodes)?;
        }

        Ok(swap_opcodes)
    }

    pub fn to_call_data(&self, opcodes: &MulticallerCalls) -> Result<(Address, Bytes)> {
        let call_data = OpcodesEncoderV2::pack_do_calls(opcodes)?;
        Ok((self.multicaller_address, call_data))
    }

    pub fn encode_swap_steps(
        &self,
        sp0: &SwapStep<LoomDataTypesEthereum>,
        sp1: &SwapStep<LoomDataTypesEthereum>,
    ) -> Result<MulticallerCalls> {
        if sp0.can_flash_swap() {
            trace!("encode_swap_steps -> sp0.can_flash_swap()");
            self.encode_in_amount(sp0.clone(), sp1.clone())
        } else if sp1.can_flash_swap() {
            trace!("encode_swap_steps -> sp1.can_flash_swap()");
            self.encode_out_amount(sp0.clone(), sp1.clone())
        } else {
            trace!("encode_swap_steps -> encode_balancer_flash_loan");
            self.encode_balancer_flash_loan(vec![sp0.clone(), sp1.clone()])
        }
    }

    fn add_calls_with_optional_value(&self, calls: &mut MulticallerCalls, call_list: Vec<(Address, Bytes, Option<U256>)>) {
        for (to, data, value) in call_list {
            if let Some(value) = value {
                calls.add(MulticallerCall::new_call_with_value(to, &data, value));
            } else {
                calls.add(MulticallerCall::new_call(to, &data));
            }
        }
    }
}
