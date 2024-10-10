use crate::MulticallerSwapEncoder;
use alloy_primitives::{Bytes, U256};
use defi_entities::{Swap, SwapEncoder, SwapStep};
use defi_types::MulticallerCalls;
use eyre::{eyre, OptionExt, Result};
use tracing::{debug, error};
use defi_entities::tips::tips_and_value_for_swap_type;

impl SwapEncoder for MulticallerSwapEncoder {
    fn encode(&self, swap: Swap, bribe: Option<U256>) -> Result<Bytes> {
        let swap_vec = match &swap {
            Swap::BackrunSwapLine(_) | Swap::BackrunSwapSteps(_) => {
                vec![swap.to_swap_steps(self.swap_step_encoder.get_multicaller()).ok_or_eyre("SWAP_TYPE_NOTE_COVERED")?]
            }
            Swap::Multiple(swap_vec) => {
                let mut ret: Vec<(SwapStep, SwapStep)> = Vec::new();
                for s in swap_vec.iter() {
                    ret.push(s.to_swap_steps(self.swap_step_encoder.get_multicaller()).ok_or_eyre("AA")?);
                }
                ret
            }
            Swap::ExchangeSwapLine(_) => vec![],
            Swap::None => {
                vec![]
            }
        };

        let swap_opcodes = if swap_vec.is_empty() {
            match &swap {
                Swap::ExchangeSwapLine(swap_line) => {
                    debug!("Swap::ExchangeSwapLine encoding started");
                    match self.swap_step_encoder.swap_line_encoder.encode_swap_line_in_amount(
                        swap_line,
                        self.swap_step_encoder.get_multicaller(),
                        self.swap_step_encoder.get_multicaller(),
                    ) {
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
            let sp0 = &swap_vec[0].0;
            let sp1 = &swap_vec[0].1;
            self.swap_step_encoder.encode_swap_steps(sp0, sp1)?
        } else {
            let mut ret = MulticallerCalls::new();
            for (sp0, sp1) in swap_vec.iter() {
                ret = self.swap_step_encoder.encode_do_calls(ret, self.swap_step_encoder.encode_swap_steps(sp0, sp1)?)?;
            }
            ret
        };


        let (tips_vec, call_value) = tips_and_value_for_swap_type(&swap, None, gas_cost, estimate_request.eth_balance)?;


        for tips in tips_vec {
            tips_opcodes =
                self.swap_step_encoder.encode_tips(tips_opcodes, tips.token_in.get_address(), tips.min_change, tips.tips, tx_signer.address())?;

        Err(eyre!("NOT_IMPLEMENTED"))
    }
}
