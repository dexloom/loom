use alloy_primitives::{Address, U256};
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use std::collections::HashMap;
use tracing::trace;

use crate::opcodes_helpers::OpcodesHelpers;
use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use loom_defi_abi::AbiEncoderHelper;
use loom_defi_address_book::TokenAddressEth;
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

        let in_native = if cur_pool.is_native() { TokenAddressEth::is_weth(&token_from_address) } else { false };
        let out_native = if cur_pool.is_native() { TokenAddressEth::is_weth(&token_to_address) } else { false };

        trace!(
            "curve swap for pool={:?} native={} amount={:?} from {} to {}",
            cur_pool.get_address(),
            cur_pool.is_native(),
            amount_in,
            token_from_address,
            token_to_address
        );

        let mut opcodes: Vec<(MulticallerCall, u32, usize)> = Vec::new();

        if in_native {
            // Swap opcode
            let mut swap_opcode = MulticallerCall::new_call_with_value(
                pool_address,
                &abi_encoder.encode_swap_in_amount_provided(
                    cur_pool,
                    token_from_address,
                    token_to_address,
                    amount_in.unwrap_or_default(),
                    multicaller,
                    payload.encode()?,
                )?,
                amount_in.unwrap_or_default(),
            );

            if !Self::need_balance(cur_pool.get_address()) {
                swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
            }

            // Withdraw WETH
            opcodes.push((
                MulticallerCall::new_call(token_from_address, &AbiEncoderHelper::encode_weth_withdraw(amount_in.unwrap_or_default())),
                0x4,
                0x20,
            ));
            opcodes.push((swap_opcode, abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(), 0x20));
        } else {
            //Approve
            opcodes.push((
                MulticallerCall::new_call(
                    token_from_address,
                    &AbiEncoderHelper::encode_erc20_approve(cur_pool.get_address(), amount_in.unwrap_or_default()),
                ),
                0x24,
                0x20,
            ));

            // SWAP
            let mut swap_opcode = MulticallerCall::new_call(
                pool_address,
                &abi_encoder.encode_swap_in_amount_provided(
                    cur_pool,
                    token_from_address,
                    token_to_address,
                    amount_in.unwrap_or_default(),
                    multicaller,
                    payload.encode()?,
                )?,
            );

            if !Self::need_balance(cur_pool.get_address()) {
                swap_opcode.set_return_stack(true, 0, 0x0, 0x20);
            }
            opcodes.push((swap_opcode, abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(), 0x20));
        }

        swap_opcodes.merge(OpcodesHelpers::build_multiple_stack(amount_in, opcodes, Some(token_from_address))?);

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

            if let PreswapRequirement::Transfer(addr) = next_pool.preswap_requirement() {
                trace!("transfer token={:?}, to={:?}, amount=stack_rel_0", token_to_address, addr);

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
