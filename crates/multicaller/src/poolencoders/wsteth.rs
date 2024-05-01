use alloy_primitives::{Address, Bytes, U256};
use eyre::{eyre, Result};
use lazy_static::lazy_static;

use defi_entities::{PoolWrapper, SwapAmountType};
use defi_types::{Opcode, Opcodes};

use crate::helpers::EncoderHelper;

pub struct WstEthSwapEncoder {}

lazy_static! {
    pub static ref WETH_ADDRESS : Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
    pub static ref STETH_ADDRESS : Address = "0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84".parse().unwrap();
    pub static ref WSTETH_ADDRESS : Address = "0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0".parse().unwrap();
}


impl WstEthSwapEncoder {
    pub fn encode_swap_in_amount_provided(token_from_address: Address, token_to_address: Address, amount_in: SwapAmountType, swap_opcodes: &mut Opcodes, cur_pool: &PoolWrapper, next_pool: Option<&PoolWrapper>, multicaller: Address) -> Result<()> {
        let pool_encoder = cur_pool.get_encoder();
        let pool_address = cur_pool.get_address();

        if token_from_address == *WETH_ADDRESS && token_to_address == *WSTETH_ADDRESS {
            match amount_in {
                SwapAmountType::Set(amount) => {
                    let mut weth_withdraw_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(amount));
                    let mut swap_opcode = Opcode::new_call_with_value(pool_address,
                                                                      &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, amount, multicaller, Bytes::new())?, amount);
                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }


                    swap_opcodes
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                }
                SwapAmountType::Stack0 => {
                    let mut weth_withdraw_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(false, 0, 0x4, 0x20);

                    let mut swap_opcode = Opcode::new_call_with_value(pool_address, &Bytes::new(), U256::ZERO);
                    swap_opcode
                        .set_call_stack(false, 0, 0, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                }

                SwapAmountType::RelativeStack(stack_offset) => {
                    let mut weth_withdraw_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, stack_offset, 0x4, 0x20);

                    let mut swap_opcode = Opcode::new_call_with_value(pool_address, &Bytes::new(), U256::ZERO);
                    swap_opcode
                        .set_call_stack(true, stack_offset, 0, 0);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                }
                SwapAmountType::Balance(addr) => {
                    let weth_balance_opcode = Opcode::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));

                    let mut weth_withdraw_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, 0, 0x4, 0x20);

                    let mut swap_opcode = Opcode::new_call_with_value(pool_address, &Bytes::new(), U256::ZERO);
                    swap_opcode
                        .set_call_stack(true, 0, 0, 0);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }


                    swap_opcodes
                        .add(weth_balance_opcode)
                        .add(weth_withdraw_opcode)
                        .add(swap_opcode);
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_WSTETH_SWAP"));
                }
            }
            return Ok(());
        }

        if token_from_address == *STETH_ADDRESS && token_to_address == *WSTETH_ADDRESS ||
            token_from_address == *WSTETH_ADDRESS && token_to_address == *STETH_ADDRESS
        {
            match amount_in {
                SwapAmountType::Set(amount) => {
                    let mut steth_approve_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, amount));
                    let mut swap_opcode = Opcode::new_call(pool_address,
                                                           &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, amount, multicaller, Bytes::new())?);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes
                        .add(steth_approve_opcode)
                        .add(swap_opcode);
                }
                SwapAmountType::Stack0 => {
                    let mut steth_approve_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, U256::ZERO));
                    steth_approve_opcode.set_call_stack(false, 0, 0x24, 0x20);

                    let mut swap_opcode = Opcode::new_call(pool_address,
                                                           &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?);
                    swap_opcode
                        .set_call_stack(false, 0, 0x4, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }


                    swap_opcodes
                        .add(steth_approve_opcode)
                        .add(swap_opcode);
                }

                SwapAmountType::RelativeStack(stack_offset) => {
                    let mut steth_approve_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, U256::ZERO));
                    steth_approve_opcode.set_call_stack(true, stack_offset, 0x24, 0x20);

                    let mut swap_opcode = Opcode::new_call(pool_address,
                                                           &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?);
                    swap_opcode
                        .set_call_stack(true, stack_offset, 0x4, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }


                    swap_opcodes
                        .add(steth_approve_opcode)
                        .add(swap_opcode);
                }
                SwapAmountType::Balance(addr) => {
                    let mut steth_balance_opcode = Opcode::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));
                    steth_balance_opcode.set_return_stack(true, 0, 0, 0x20);


                    let mut steth_approve_opcode = Opcode::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, U256::ZERO));
                    steth_approve_opcode.set_call_stack(true, 0, 0x24, 0x20);

                    let mut swap_opcode = Opcode::new_call(pool_address,
                                                           &pool_encoder.encode_swap_in_amount_provided(token_from_address, token_to_address, U256::ZERO, multicaller, Bytes::new())?);
                    swap_opcode
                        .set_call_stack(true, 0, 0x4, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }


                    swap_opcodes
                        .add(steth_balance_opcode)
                        .add(steth_approve_opcode)
                        .add(swap_opcode);
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_WSTETH_SWAP"));
                }
            }
            return Ok(());
        }

        return Err(eyre!("CANNOT_ENCODE_WSTETH_SWAP"));
    }
}
