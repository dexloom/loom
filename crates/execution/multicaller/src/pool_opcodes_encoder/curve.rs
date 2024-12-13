use alloy_primitives::{Address, U256};
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use std::collections::HashMap;
use tracing::{error, trace};

use crate::helpers::AbiEncoderHelper;
use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::Pool;
use loom_types_entities::{PreswapRequirement, SwapAmountType};

pub struct CurveSwapOpcodesEncoder;

lazy_static! {
    static ref NEED_BALANCE_MAP : HashMap<Address, bool> = {
        let mut hm = HashMap::new();
        hm.insert("0xD51a44d3FaE010294C616388b506AcdA1bfAAE46".parse::<Address>().unwrap(), true);
        hm.insert("0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".parse::<Address>().unwrap(), true);
        hm.insert("0xA5407eAE9Ba41422680e2e00537571bcC53efBfD".parse::<Address>().unwrap(), true); // sUSD
        hm
    };
}

impl CurveSwapOpcodesEncoder {
    fn need_balance(address: Address) -> bool {
        *NEED_BALANCE_MAP.get(&address).unwrap_or(&false)
    }
}

impl SwapOpcodesEncoderTrait for CurveSwapOpcodesEncoder {
    #[allow(clippy::too_many_arguments)]
    fn encode_swap_in_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_in: SwapAmountType,
        cur_pool: &dyn Pool,
        next_pool: Option<&dyn Pool>,
        payload: MulticallerOpcodesPayload,
        multicaller: Address,
    ) -> Result<()> {
        //let pool_encoder = abi_encoder.cur_pool.get_encoder().ok_or_eyre("NO_POOL_ENCODER")?;
        let pool_address = cur_pool.get_address();

        let in_native = if abi_encoder.is_native(cur_pool) { AbiEncoderHelper::is_weth(token_from_address) } else { false };
        let out_native = if abi_encoder.is_native(cur_pool) { AbiEncoderHelper::is_weth(token_to_address) } else { false };

        match amount_in {
            SwapAmountType::Set(amount) => {
                if in_native {
                    let weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &AbiEncoderHelper::encode_weth_withdraw(amount));

                    trace!(
                        "curve encode_swap_in_amount_provided native set amount={} pool={} from={} to={} recipient={} payload_empty={}",
                        amount,
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call_with_value(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            amount,
                            multicaller,
                            payload.encode()?,
                        )?,
                        amount,
                    );
                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }
                    swap_opcodes.add(weth_withdraw_opcode).add(swap_opcode);
                } else {
                    trace!("approve  token={:?}, to={:?}, amount={}", token_from_address, cur_pool.get_address(), amount);

                    let approve_opcode = MulticallerCall::new_call(
                        token_from_address,
                        &AbiEncoderHelper::encode_erc20_approve(cur_pool.get_address(), amount),
                    );

                    trace!(
                        "curve encode_swap_in_amount_provided set amount={} pool={} from={} to={} recipient={} payload_empty={}",
                        amount,
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            amount,
                            multicaller,
                            payload.encode()?,
                        )?,
                    );

                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }

                    swap_opcodes.add(approve_opcode).add(swap_opcode);
                }
            }
            SwapAmountType::Stack0 => {
                if in_native {
                    let mut weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &AbiEncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(false, 0, 0x4, 0x20);

                    trace!(
                        "curve encode_swap_in_amount_provided native else for stack0 amount=stack_rel_0 pool={:?} from={} to={} recipient={} payload_empty={}",
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call_with_value(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            payload.encode()?,
                        )?,
                        U256::ZERO,
                    );
                    swap_opcode.set_call_stack(
                        false,
                        0,
                        abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                        0x20,
                    );
                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }
                    swap_opcodes.add(weth_withdraw_opcode).add(swap_opcode);
                } else {
                    trace!("approve  token={:?}, to={:?}, amount=stack_no_rel_0", token_from_address, cur_pool.get_address());

                    let mut approve_opcode = MulticallerCall::new_call(
                        token_from_address,
                        &AbiEncoderHelper::encode_erc20_approve(cur_pool.get_address(), U256::ZERO),
                    );
                    approve_opcode.set_call_stack(false, 0, 0x24, 0x20);

                    trace!(
                        "curve encode_swap_in_amount_provided else for stack0 amount=stack_rel_0 pool={:?} from={} to={} recipient={} payload_empty={}",
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            payload.encode()?,
                        )?,
                    );
                    swap_opcode.set_call_stack(
                        false,
                        0,
                        abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                        0x20,
                    );
                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }

                    swap_opcodes.add(approve_opcode).add(swap_opcode);
                }
            }
            SwapAmountType::RelativeStack(stack_offset) => {
                if in_native {
                    let mut weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &AbiEncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, stack_offset, 0x4, 0x20);

                    trace!(
                        "curve encode_swap_in_amount_provided native else for relstack amount=stack_rel_{} pool={:?} from={} to={} recipient={} payload_empty={}",
                        stack_offset,
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call_with_value(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            payload.encode()?,
                        )?,
                        U256::ZERO,
                    );
                    swap_opcode.set_call_stack(
                        true,
                        stack_offset,
                        abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                        0x20,
                    );
                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }
                    swap_opcodes.add(weth_withdraw_opcode).add(swap_opcode);
                } else {
                    trace!("approve  token={:?}, to={:?}, amount=stack_rel_{}", token_from_address, cur_pool.get_address(), stack_offset);

                    let mut approve_opcode = MulticallerCall::new_call(
                        token_from_address,
                        &AbiEncoderHelper::encode_erc20_approve(cur_pool.get_address(), U256::ZERO),
                    );
                    approve_opcode.set_call_stack(true, stack_offset, 0x24, 0x20);

                    trace!(
                        "curve encode_swap_in_amount_provided  for relstack amount=stack_rel_{} pool={:?} from={} to={} recipient={} payload_empty={}",
                        stack_offset,
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            payload.encode()?,
                        )?,
                    );
                    swap_opcode.set_call_stack(
                        true,
                        stack_offset,
                        abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                        0x20,
                    );
                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }

                    swap_opcodes.add(approve_opcode).add(swap_opcode);
                }
            }
            SwapAmountType::Balance(addr) => {
                let mut balance_opcode =
                    MulticallerCall::new_static_call(token_from_address, &AbiEncoderHelper::encode_erc20_balance_of(addr));
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);

                if in_native {
                    let mut weth_withdraw_opcode =
                        MulticallerCall::new_call(token_from_address, &AbiEncoderHelper::encode_weth_withdraw(U256::ZERO));
                    weth_withdraw_opcode.set_call_stack(true, 0, 0x4, 0x20);

                    trace!(
                        "curve encode_swap_in_amount_provided native else for balance amount=stack_rel_0 pool={:?} from={} to={} recipient={} payload_empty={}",
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call_with_value(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            payload.encode()?,
                        )?,
                        U256::ZERO,
                    );
                    swap_opcode.set_call_stack(
                        true,
                        0,
                        abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                        0x20,
                    );
                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }
                    swap_opcodes.add(balance_opcode).add(weth_withdraw_opcode).add(swap_opcode);
                } else {
                    trace!("approve  token={:?}, to={:?}, amount=balance_stack_rel_0", token_from_address, cur_pool.get_address());

                    let mut approve_opcode = MulticallerCall::new_call(
                        token_from_address,
                        &AbiEncoderHelper::encode_erc20_approve(cur_pool.get_address(), U256::ZERO),
                    );
                    approve_opcode.set_call_stack(true, 0, 0x24, 0x20);

                    trace!(
                        "curve encode_swap_in_amount_provided else for balance amount=stack_rel_0 pool={:?} from={} to={} recipient={} payload_empty={}",
                        cur_pool.get_address(),
                        token_from_address,
                        token_to_address,
                        multicaller,
                        payload.is_empty()
                    );

                    let mut swap_opcode = MulticallerCall::new_call(
                        pool_address,
                        &abi_encoder.encode_swap_in_amount_provided(
                            cur_pool,
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            multicaller,
                            payload.encode()?,
                        )?,
                    );
                    swap_opcode.set_call_stack(
                        true,
                        0,
                        abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                        0x20,
                    );

                    if !Self::need_balance(cur_pool.get_address()) {
                        swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
                    }

                    swap_opcodes.add(balance_opcode).add(approve_opcode).add(swap_opcode);
                }
            }
            _ => {
                error!("Curve amount not handled")
            }
        }

        if out_native {
            let mut weth_deposit_opcode =
                MulticallerCall::new_call_with_value(token_to_address, &AbiEncoderHelper::encode_weth_deposit(), U256::ZERO);
            weth_deposit_opcode.set_call_stack(true, 0, 0x0, 0x0);
            swap_opcodes.add(weth_deposit_opcode);
        }

        if let Some(next_pool) = next_pool {
            if Self::need_balance(cur_pool.get_address()) {
                let mut balance_opcode =
                    MulticallerCall::new_static_call(token_to_address, &AbiEncoderHelper::encode_erc20_balance_of(multicaller));
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
                swap_opcodes.add(balance_opcode);
            }

            if let PreswapRequirement::Transfer(addr) = abi_encoder.preswap_requirement(next_pool) {
                let mut transfer_opcode =
                    MulticallerCall::new_call(token_to_address, &AbiEncoderHelper::encode_erc20_transfer(addr, U256::ZERO));
                transfer_opcode.set_call_stack(true, 0, 0x24, 0x20);
                swap_opcodes.add(transfer_opcode);
            }
        }
        Ok(())
    }

    fn encode_swap_out_amount_provided(
        &self,
        _swap_opcodes: &mut MulticallerCalls,
        _abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        _token_from_address: Address,
        _token_to_address: Address,
        _amount_out: SwapAmountType,
        _cur_pool: &dyn Pool,
        _next_pool: Option<&dyn Pool>,
        _payload: MulticallerOpcodesPayload,
        _multicaller_address: Address,
    ) -> Result<()> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }
}
