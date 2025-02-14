use alloy_primitives::{Address, U256};
use eyre::Result;
use loom_defi_abi::AbiEncoderHelper;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::SwapAmountType;

pub struct OpcodesHelpers {}

impl OpcodesHelpers {
    pub fn build_log_stack(size: usize) -> Result<MulticallerCalls> {
        let mut calls = MulticallerCalls::new();

        for i in 0..size {
            calls.add(MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_log_stack_offset(U256::from(i))));
        }

        Ok(calls)
    }

    pub fn build_call_stack(
        amount: SwapAmountType,
        call: MulticallerCall,
        offset: u32,
        len: usize,
        balance_of_token: Option<Address>,
    ) -> Result<MulticallerCalls> {
        let mut calls = MulticallerCalls::new();
        let mut call = call;

        match amount {
            SwapAmountType::Set(_value) => {}
            SwapAmountType::Balance(balance_of_owner) => {
                let mut balance_opcode = MulticallerCall::new_static_call(
                    balance_of_token.unwrap(),
                    &AbiEncoderHelper::encode_erc20_balance_of(balance_of_owner),
                );
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
                calls.add(balance_opcode);

                call.set_call_stack(true, 0, offset, len);
            }
            SwapAmountType::RelativeStack(stack_offset) => {
                call.set_call_stack(true, stack_offset, offset, len);
            }
            SwapAmountType::NotSet => {
                call.set_call_stack(true, 0, offset, len);
            }
            SwapAmountType::Stack0 => {
                call.set_call_stack(false, 0, offset, len);
            }
        }
        calls.add(call);

        Ok(calls)
    }

    pub fn build_multiple_stack(
        amount: SwapAmountType,
        calls: Vec<(MulticallerCall, u32, usize)>,
        balance_of_token: Option<Address>,
    ) -> Result<MulticallerCalls> {
        let mut multicaller_calls = MulticallerCalls::new();

        if let SwapAmountType::Balance(balance_of_owner) = amount {
            let mut balance_opcode =
                MulticallerCall::new_static_call(balance_of_token.unwrap(), &AbiEncoderHelper::encode_erc20_balance_of(balance_of_owner));
            balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
            multicaller_calls.add(balance_opcode);
        }

        for (call, offset, len) in calls {
            let mut call = call;
            match amount {
                SwapAmountType::Set(_value) => {}
                SwapAmountType::Balance(_balance_of_owner) => {
                    call.set_call_stack(true, 0, offset, len);
                }
                SwapAmountType::RelativeStack(stack_offset) => {
                    call.set_call_stack(true, stack_offset, offset, len);
                }
                SwapAmountType::NotSet => {
                    call.set_call_stack(true, 0, offset, len);
                }
                SwapAmountType::Stack0 => {
                    call.set_call_stack(false, 0, offset, len);
                }
            }
            multicaller_calls.add(call);
        }

        Ok(multicaller_calls)
    }
}
