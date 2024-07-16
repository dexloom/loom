use std::collections::HashMap;

use alloy_primitives::{Address, Bytes, U256};
use eyre::Result;
use lazy_static::lazy_static;
use log::error;

use defi_entities::{PreswapRequirement, SwapAmountType};
use defi_entities::PoolWrapper;
use defi_types::{MulticallerCall, MulticallerCalls};

use crate::helpers::EncoderHelper;

pub struct CurveSwapEncoder {}

lazy_static! {
    static ref NEED_BALANCE_MAP : HashMap<Address, bool> = {
        let mut hm = HashMap::new();
        hm.insert("0xD51a44d3FaE010294C616388b506AcdA1bfAAE46".parse::<Address>().unwrap(), true);
        hm.insert("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".parse::<Address>().unwrap(), true);
        hm.insert("0xA5407eAE9Ba41422680e2e00537571bcC53efBfD".parse::<Address>().unwrap(), true); // sUSD
        hm
    };
}

impl CurveSwapEncoder {
    fn need_balance(address: Address) -> bool {
        *NEED_BALANCE_MAP.get(&address).unwrap_or(&false)
    }

    pub fn encode_swap_in_amount_provided(token_from_address: Address, token_to_address: Address, amount_in: SwapAmountType, swap_opcodes: &mut MulticallerCalls, cur_pool: &PoolWrapper, next_pool: Option<&PoolWrapper>, multicaller: Address) -> Result<()> {
        let pool_encoder = cur_pool.get_encoder();
        let pool_address = cur_pool.get_address();

        let in_native = if pool_encoder.is_native() { EncoderHelper::is_weth(token_from_address) } else { false };
        let out_native = if pool_encoder.is_native() { EncoderHelper::is_weth(token_to_address) } else { false };

        match amount_in {
            SwapAmountType::Set(amount) => {
                if in_native {
                    let weth_withdraw_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(amount));
                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address,
                                                                               &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, amount, multicaller, Bytes::new())?, amount);
                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }
                    swap_opcodes
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                } else {
                    let approve_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(cur_pool.get_address(), amount));
                    let mut swap_opcode = MulticallerCall::new_call(pool_address,
                                                                    &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, amount, multicaller, Bytes::new())?);

                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }

                    swap_opcodes
                        .add(approve_opcode)
                        .add(swap_opcode);
                }
            }
            SwapAmountType::Stack0 => {
                if in_native {
                    let mut weth_withdraw_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(false, 0, 0x4, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address,
                                                                               &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?, U256::ZERO);
                    swap_opcode
                        .set_call_stack(false, 0, pool_encoder.swap_in_amount_offset(token_from_address, token_to_address).unwrap(), 0x20);
                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }
                    swap_opcodes
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                } else {
                    let mut approve_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(cur_pool.get_address(), U256::ZERO));
                    approve_opcode.set_call_stack(false, 0, 0x24, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call(pool_address,
                                                                    &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?);
                    swap_opcode
                        .set_call_stack(false, 0, pool_encoder.swap_in_amount_offset(token_from_address, token_to_address).unwrap(), 0x20);
                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }

                    swap_opcodes
                        .add(approve_opcode)
                        .add(swap_opcode);
                }
            }
            SwapAmountType::RelativeStack(stack_offset) => {
                if in_native {
                    let mut weth_withdraw_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, stack_offset, 0x4, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address,
                                                                               &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?, U256::ZERO);
                    swap_opcode
                        .set_call_stack(true, stack_offset, pool_encoder.swap_in_amount_offset(token_from_address, token_to_address).unwrap(), 0x20);
                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }
                    swap_opcodes
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                } else {
                    let mut approve_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(cur_pool.get_address(), U256::ZERO));
                    approve_opcode.set_call_stack(true, stack_offset, 0x24, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call(pool_address,
                                                                    &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?);
                    swap_opcode
                        .set_call_stack(true, stack_offset, pool_encoder.swap_in_amount_offset(token_from_address, token_to_address).unwrap(), 0x20);
                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }

                    swap_opcodes
                        .add(approve_opcode)
                        .add(swap_opcode);
                }
            }
            SwapAmountType::Balance(addr) => {
                let mut balance_opcode = MulticallerCall::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);

                if in_native {
                    let mut weth_withdraw_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, 0, 0x4, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address,
                                                                               &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?, U256::ZERO);
                    swap_opcode
                        .set_call_stack(true, 0, pool_encoder.swap_in_amount_offset(token_from_address, token_to_address).unwrap(), 0x20);
                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }
                    swap_opcodes
                        .add(balance_opcode)
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                } else {
                    let mut approve_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(cur_pool.get_address(), U256::ZERO));
                    approve_opcode.set_call_stack(true, 0, 0x24, 0x20);


                    let mut swap_opcode = MulticallerCall::new_call(pool_address,
                                                                    &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?);
                    swap_opcode
                        .set_call_stack(true, 0, pool_encoder.swap_in_amount_offset(token_from_address, token_to_address).unwrap(), 0x20);

                    if !Self::need_balance(cur_pool.get_address()) { swap_opcode.set_return_stack(true, 0, 0x0, 0x20); }

                    swap_opcodes
                        .add(balance_opcode)
                        .add(approve_opcode)
                        .add(swap_opcode);
                }
            }
            _ => {
                error!("Curve amount not handled")
            }
        }

        if out_native {
            let mut weth_deposit_opcode = MulticallerCall::new_call_with_value(token_to_address, &EncoderHelper::encode_weth_deposit(), U256::ZERO);
            weth_deposit_opcode.set_call_stack(true, 0, 0x0, 0x0);
            swap_opcodes.add(weth_deposit_opcode);
        }


        if let Some(next_pool) = next_pool {
            if Self::need_balance(cur_pool.get_address()) {
                let mut balance_opcode = MulticallerCall::new_static_call(token_to_address, &EncoderHelper::encode_erc20_balance_of(multicaller));
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
                swap_opcodes.add(balance_opcode);
            }

            if let PreswapRequirement::Transfer(addr) = next_pool.get_encoder().preswap_requirement() {
                let mut transfer_opcode = MulticallerCall::new_call(token_to_address, &EncoderHelper::encode_erc20_transfer(addr, U256::ZERO));
                transfer_opcode.set_call_stack(true, 0, 0x24, 0x20);
                swap_opcodes.add(transfer_opcode);
            }
        }
        Ok(())
    }
}