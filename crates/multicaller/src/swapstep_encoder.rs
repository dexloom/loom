use alloy_primitives::{Address, Bytes, U256};
use eyre::Result;
use lazy_static::lazy_static;
use log::{debug, trace};

use defi_entities::{SwapAmountType, SwapStep};
use defi_types::{MulticallerCall, MulticallerCalls};

use crate::helpers::EncoderHelper;
use crate::opcodes_encoder::{OpcodesEncoder, OpcodesEncoderV2};
use crate::SwapPathEncoder;

lazy_static! {
    static ref BALANCER_VAULT_ADDRESS: Address = "0xBA12222222228d8Ba445958a75a0704d566BF2C8".parse().unwrap();
}

#[derive(Clone)]
pub struct SwapStepEncoder {
    multicaller: Address,
    swap_path_encoder: SwapPathEncoder,
}

impl SwapStepEncoder {
    pub fn new(multicaller: Address) -> Self {
        Self { multicaller, swap_path_encoder: SwapPathEncoder::new(multicaller) }
    }

    pub fn get_multicaller(&self) -> Address {
        self.multicaller
    }

    pub fn encode_do_calls(&self, opcodes: MulticallerCalls, inside_opcodes: MulticallerCalls) -> Result<MulticallerCalls> {
        let mut opcodes = opcodes;
        let call_bytes = OpcodesEncoderV2::pack_do_calls(&inside_opcodes)?;
        opcodes.add(MulticallerCall::new_call(self.multicaller, &call_bytes));
        Ok(opcodes)
    }

    pub fn encode_tips(
        &self,
        swap_opcodes: MulticallerCalls,
        token_address: Address,
        min_balance: U256,
        tips: U256,
        to: Address,
    ) -> Result<MulticallerCalls> {
        self.swap_path_encoder.encode_tips(swap_opcodes, token_address, min_balance, tips, to)
    }

    pub fn encode_balancer_flash_loan(&self, steps: Vec<SwapStep>) -> Result<MulticallerCalls> {
        let flash_funds_to = self.multicaller;

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
                swap_opcodes.merge(self.swap_path_encoder.encode_swap_line_in_amount(
                    swap.swap_line_vec().first().unwrap(),
                    flash_funds_to,
                    self.multicaller,
                )?);
            } else {
                for swap_path in swap.swap_line_vec().iter() {
                    let opcodes = self.swap_path_encoder.encode_swap_line_in_amount(swap_path, flash_funds_to, self.multicaller)?;
                    let call_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes)?;
                    swap_opcodes.add(MulticallerCall::new_call(self.multicaller, &call_bytes));
                }
            }
        }

        let inside_call_bytes = OpcodesEncoderV2::pack_do_calls_data(&swap_opcodes)?;

        let mut flash_opcodes = MulticallerCalls::new();

        let flash_call_data = EncoderHelper::encode_balancer_flashloan(token.get_address(), in_amount, inside_call_bytes, self.multicaller);

        flash_opcodes.add(MulticallerCall::new_call(*BALANCER_VAULT_ADDRESS, &flash_call_data));

        Ok(flash_opcodes)
    }

    pub fn encode_in_amount(&self, step0: SwapStep, step1: SwapStep) -> Result<MulticallerCalls> {
        let flash = step0.clone();
        let mut swap = step1.clone();

        let flash_funds_to = self.multicaller;

        if flash.len() > 1 || swap.len() > 1 {
            swap.get_mut_swap_line_by_index(swap.len() - 1).amount_in = SwapAmountType::Balance(flash_funds_to);
        }

        trace!("funds_to {:?}", flash_funds_to);

        let mut swap_opcodes = MulticallerCalls::new();

        if swap.swap_line_vec().len() == 1 {
            swap_opcodes.merge(self.swap_path_encoder.encode_swap_line_in_amount(
                swap.swap_line_vec().first().unwrap(),
                flash_funds_to,
                self.multicaller,
            )?);
        } else {
            for swap_path in swap.swap_line_vec().iter() {
                let opcodes = self.swap_path_encoder.encode_swap_line_in_amount(swap_path, flash_funds_to, self.multicaller)?;
                let call_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes)?;
                swap_opcodes.add(MulticallerCall::new_call(self.multicaller, &call_bytes));

                //swap_opcodes.merge( self.swap_path_encoder.encode_swap_in_amount(swap_path, flash_funds_to, self.multicaller)?);
                //let pop_opcode = Opcode::new_calculation_call(&Bytes::from(vec![0x8,0x8,0x11,0]));
                //swap_opcodes.add(pop_opcode);
            }
        }

        let mut flash_swaps = flash.swap_line_vec().clone();
        flash_swaps.reverse();

        for flash_swap_path in flash_swaps.iter() {
            swap_opcodes = self.swap_path_encoder.encode_flash_swap_line_in_amount(flash_swap_path, swap_opcodes, flash_funds_to)?;
        }

        Ok(swap_opcodes)
    }

    pub fn encode_out_amount(&self, step0: SwapStep, step1: SwapStep) -> Result<MulticallerCalls> {
        let flash = step1.clone();
        let swap = step0.clone();

        let flash_funds_to = self.multicaller;

        debug!("funds_to {:?}", flash_funds_to);

        let mut swap_opcodes = MulticallerCalls::new();

        if swap.swap_line_vec().len() == 1 {
            swap_opcodes.merge(self.swap_path_encoder.encode_swap_line_in_amount(
                swap.swap_line_vec().first().unwrap(),
                flash_funds_to,
                self.multicaller,
            )?);
        } else {
            for swap_path in swap.swap_line_vec().iter() {
                let opcodes = self.swap_path_encoder.encode_swap_line_in_amount(swap_path, flash_funds_to, self.multicaller)?;
                let call_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes)?;
                swap_opcodes.add(MulticallerCall::new_call(self.multicaller, &call_bytes));

                //swap_opcodes.merge( self.swap_path_encoder.encode_swap_in_amount(swap_path, flash_funds_to, self.multicaller)?);
                //let pop_opcode = Opcode::new_calculation_call(&Bytes::from(vec![0x8,0x8,0x11,0]));
                //swap_opcodes.add(pop_opcode);
            }
        }

        let mut flash_swaps = flash.swap_line_vec().clone();
        flash_swaps.reverse();

        for flash_swap_path in flash_swaps.iter() {
            swap_opcodes = self.swap_path_encoder.encode_flash_swap_line_out_amount(flash_swap_path, swap_opcodes, flash_funds_to)?;
        }

        Ok(swap_opcodes)
    }

    pub fn to_call_data(&self, opcodes: &MulticallerCalls) -> Result<(Address, Bytes)> {
        let call_data = OpcodesEncoderV2::pack_do_calls(opcodes)?;
        Ok((self.multicaller, call_data))
    }

    pub fn encode(&self, sp0: &SwapStep, sp1: &SwapStep) -> Result<MulticallerCalls> {
        if sp0.can_flash_swap() {
            self.encode_in_amount(sp0.clone(), sp1.clone())
        } else if sp1.can_flash_swap() {
            self.encode_out_amount(sp0.clone(), sp1.clone())
        } else {
            self.encode_balancer_flash_loan(vec![sp0.clone(), sp1.clone()])
        }
    }
}
