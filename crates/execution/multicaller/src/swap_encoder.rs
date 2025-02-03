use crate::MulticallerSwapEncoder;
use alloy_primitives::{Address, BlockNumber, Bytes, U256};
use eyre::{eyre, OptionExt, Result};
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_blockchain::MulticallerCalls;
use loom_types_entities::tips::{tips_and_value_for_swap_type, Tips};
use loom_types_entities::{Swap, SwapEncoder, SwapStep};
use tracing::{debug, error, trace};

impl SwapEncoder for MulticallerSwapEncoder {
    fn set_address(&mut self, multicaller_address: Address) {
        self.multicaller_address = multicaller_address;
        self.swap_step_encoder.multicaller_address = multicaller_address;
        self.swap_step_encoder.swap_line_encoder.multicaller_address = multicaller_address;
    }

    fn address(&self) -> Address {
        self.multicaller_address
    }

    fn encode(
        &self,
        swap: Swap,
        tips_pct: Option<u32>,
        _next_block_number: Option<BlockNumber>,
        gas_cost: Option<U256>,
        sender_address: Option<Address>,
        sender_eth_balance: Option<U256>,
    ) -> Result<(Address, Option<U256>, Bytes, Vec<Tips>)> {
        let swap_vec = match &swap {
            Swap::BackrunSwapLine(_) | Swap::BackrunSwapSteps(_) => {
                vec![swap.to_swap_steps(self.swap_step_encoder.get_contract_address()).ok_or_eyre("SWAP_TYPE_NOTE_COVERED")?]
            }
            Swap::Multiple(swap_vec) => {
                let mut ret: Vec<(SwapStep<LoomDataTypesEthereum>, SwapStep<LoomDataTypesEthereum>)> = Vec::new();
                for s in swap_vec.iter() {
                    ret.push(s.to_swap_steps(self.swap_step_encoder.get_contract_address()).ok_or_eyre("AA")?);
                }
                ret
            }
            Swap::ExchangeSwapLine(_) => vec![],
            Swap::None => {
                vec![]
            }
        };

        let mut swap_steps = "".to_string();
        for (level, (sp0, sp1)) in swap_vec.iter().enumerate() {
            swap_steps.push_str(format!("{}{} \n{}\t-> {}\n", "\t".repeat(level), sp0, "\t".repeat(level), sp1).as_str());
        }
        debug!("Swaps:\n{}", swap_steps);

        let mut swap_opcodes = if swap_vec.is_empty() {
            match &swap {
                Swap::ExchangeSwapLine(swap_line) => {
                    trace!("START: exchange swap line");
                    match self.swap_step_encoder.swap_line_encoder.encode_swap_line_in_amount(swap_line, None) {
                        Ok(calls) => calls,
                        Err(e) => {
                            error!("swap_line_encoder.encode_swap_line_in_amount : {}", e);
                            return Err(eyre!("ENCODING_FAILED"));
                        }
                    }
                }
                _ => return Err(eyre!("NO_SWAP_STEPS")),
            }
        } else if swap_vec.len() == 1 {
            trace!("START: encode_swap_steps two-hop");
            let sp0 = &swap_vec[0].0;
            let sp1 = &swap_vec[0].1;
            self.swap_step_encoder.encode_swap_steps(sp0, sp1)?
        } else {
            trace!("START: encode_swap_steps multi-hop");
            let mut ret = MulticallerCalls::new();
            for (sp0, sp1) in swap_vec.iter() {
                ret = self.swap_step_encoder.encode_do_calls(ret, self.swap_step_encoder.encode_swap_steps(sp0, sp1)?)?;
            }
            ret
        };
        trace!("END: swap_opcodes");

        let tips_vec =
            if let (Some(tips_pct), Some(sender_address), Some(sender_eth_balance)) = (tips_pct, sender_address, sender_eth_balance) {
                let (tips_vec, _call_value) = tips_and_value_for_swap_type(&swap, Some(tips_pct), gas_cost, sender_eth_balance)?;
                for tips in &tips_vec {
                    swap_opcodes = self.swap_step_encoder.encode_tips(
                        swap_opcodes,
                        tips.token_in.get_address(),
                        tips.min_change,
                        tips.tips,
                        sender_address,
                    )?;
                }
                tips_vec
            } else {
                vec![]
            };

        let (to, call_data) = self.swap_step_encoder.to_call_data(&swap_opcodes)?;

        Ok((to, None, call_data, tips_vec))
    }
}
