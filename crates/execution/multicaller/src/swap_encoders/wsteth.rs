use crate::abi_encoders::ProtocolAbiSwapEncoderTrait;
use crate::helpers::EncoderHelper;
use alloy_primitives::{Address, Bytes, U256};
use eyre::{eyre, Result};
use loom_defi_address_book::TokenAddressEth;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{PoolWrapper, SwapAmountType};

pub struct WstEthSwapEncoder {}

impl WstEthSwapEncoder {
    #[allow(clippy::too_many_arguments)]
    pub fn encode_swap_in_amount_provided<E: ProtocolAbiSwapEncoderTrait>(
        abi_encoder: &E,
        token_from_address: Address,
        token_to_address: Address,
        amount_in: SwapAmountType,
        swap_opcodes: &mut MulticallerCalls,
        cur_pool: &PoolWrapper,
        next_pool: Option<&PoolWrapper>,
        multicaller: Address,
    ) -> Result<()> {
        let pool_address = cur_pool.get_address();

        if token_from_address == TokenAddressEth::WETH && token_to_address == TokenAddressEth::WSTETH {
            match amount_in {
                SwapAmountType::Set(amount) => {
                    let weth_withdraw_opcode = MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(amount));
                    let mut swap_opcode = MulticallerCall::new_call_with_value(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            &**cur_pool,
                            token_from_address,
                            token_to_address,
                            amount,
                            multicaller,
                            Bytes::new(),
                        )?,
                        amount,
                    );
                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(weth_withdraw_opcode).add(swap_opcode);
                }
                SwapAmountType::Stack0 => {
                    let mut weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(false, 0, 0x4, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address, &Bytes::new(), U256::ZERO);
                    swap_opcode.set_call_stack(false, 0, 0, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(weth_withdraw_opcode).add(swap_opcode);
                }

                SwapAmountType::RelativeStack(stack_offset) => {
                    let mut weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, stack_offset, 0x4, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address, &Bytes::new(), U256::ZERO);
                    swap_opcode.set_call_stack(true, stack_offset, 0, 0);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(weth_withdraw_opcode).add(swap_opcode);
                }
                SwapAmountType::Balance(addr) => {
                    let weth_balance_opcode =
                        MulticallerCall::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));

                    let mut weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, 0, 0x4, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call_with_value(pool_address, &Bytes::new(), U256::ZERO);
                    swap_opcode.set_call_stack(true, 0, 0, 0);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(weth_balance_opcode).add(weth_withdraw_opcode).add(swap_opcode);
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_WSTETH_SWAP"));
                }
            }
            return Ok(());
        }

        if token_from_address == TokenAddressEth::STETH && token_to_address == TokenAddressEth::WSTETH
            || token_from_address == TokenAddressEth::WSTETH && token_to_address == TokenAddressEth::STETH
        {
            match amount_in {
                SwapAmountType::Set(amount) => {
                    let steth_approve_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, amount));
                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            &**cur_pool,
                            token_from_address,
                            token_to_address,
                            amount,
                            multicaller,
                            Bytes::new(),
                        )?,
                    );

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(steth_approve_opcode).add(swap_opcode);
                }
                SwapAmountType::Stack0 => {
                    let mut steth_approve_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, U256::ZERO));
                    steth_approve_opcode.set_call_stack(false, 0, 0x24, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            &**cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            Bytes::new(),
                        )?,
                    );
                    swap_opcode.set_call_stack(false, 0, 0x4, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(steth_approve_opcode).add(swap_opcode);
                }

                SwapAmountType::RelativeStack(stack_offset) => {
                    let mut steth_approve_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, U256::ZERO));
                    steth_approve_opcode.set_call_stack(true, stack_offset, 0x24, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            &**cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            Bytes::new(),
                        )?,
                    );
                    swap_opcode.set_call_stack(true, stack_offset, 0x4, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(steth_approve_opcode).add(swap_opcode);
                }
                SwapAmountType::Balance(addr) => {
                    let mut steth_balance_opcode =
                        MulticallerCall::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));
                    steth_balance_opcode.set_return_stack(true, 0, 0, 0x20);

                    let mut steth_approve_opcode =
                        MulticallerCall::new_call(token_from_address, &EncoderHelper::encode_erc20_approve(token_to_address, U256::ZERO));
                    steth_approve_opcode.set_call_stack(true, 0, 0x24, 0x20);

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            &**cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            Bytes::new(),
                        )?,
                    );
                    swap_opcode.set_call_stack(true, 0, 0x4, 0x20);

                    if next_pool.is_some() {
                        swap_opcode.set_return_stack(true, 0, 0, 0x20);
                    }

                    swap_opcodes.add(steth_balance_opcode).add(steth_approve_opcode).add(swap_opcode);
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_WSTETH_SWAP"));
                }
            }
            return Ok(());
        }

        Err(eyre!("CANNOT_ENCODE_WSTETH_SWAP"))
    }
}
