use crate::opcodes_helpers::OpcodesHelpers;
use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use alloy_primitives::{Address, Bytes, U256};
use eyre::{eyre, OptionExt};
use loom_defi_abi::AbiEncoderHelper;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{Pool, PreswapRequirement, SwapAmountType};
use tracing::trace;

pub struct UniswapV3SwapOpcodesEncoder {}

impl SwapOpcodesEncoderTrait for UniswapV3SwapOpcodesEncoder {
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
        multicaller_address: Address,
    ) -> eyre::Result<()> {
        let inside_call_payload = if payload.is_empty() { Bytes::from(token_from_address.to_vec()) } else { payload.encode()? };

        let swap_to: Address = if let Some(next_pool) = next_pool {
            match next_pool.preswap_requirement() {
                PreswapRequirement::Transfer(next_funds_to) => next_funds_to,
                _ => multicaller_address,
            }
        } else {
            multicaller_address
        };

        let mut swap_opcode = MulticallerCall::new_call(
            cur_pool.get_address(),
            &abi_encoder.encode_swap_in_amount_provided(
                cur_pool,
                token_from_address,
                token_to_address,
                amount_in.unwrap_or_default(),
                swap_to,
                inside_call_payload,
            )?,
        );

        swap_opcode.set_return_stack(
            true,
            0,
            abi_encoder.swap_in_amount_return_offset(cur_pool, token_from_address, token_to_address).unwrap(),
            0x20,
        );

        swap_opcodes.merge(OpcodesHelpers::build_call_stack(
            amount_in,
            swap_opcode,
            abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).ok_or_eyre("NO_OFFSET")?,
            0x20,
            Some(token_from_address),
        )?);

        if next_pool.is_some() {
            trace!("has next pool");
            if let Some(x) = abi_encoder.swap_in_amount_return_script(cur_pool, token_from_address, token_to_address) {
                let calc_opcode = MulticallerCall::new_calculation_call(&x);
                swap_opcodes.add(calc_opcode);
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
    ) -> eyre::Result<()> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    fn encode_flash_swap_in_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_in: SwapAmountType,
        flash_pool: &dyn Pool,
        prev_pool: Option<&dyn Pool>,
        payload: MulticallerOpcodesPayload,
        multicaller_address: Address,
    ) -> eyre::Result<()> {
        let swap_to = match prev_pool {
            Some(prev_pool) => match prev_pool.preswap_requirement() {
                PreswapRequirement::Transfer(funds_to) => {
                    trace!("other transfer to previous pool: funds_to={:?}, prev_pool={:?}", funds_to, prev_pool.get_address());
                    funds_to
                }
                _ => {
                    trace!("other swap to multicaller, prev_pool={:?}", prev_pool.get_address());
                    multicaller_address
                }
            },
            None => multicaller_address,
        };

        let payload = match payload {
            MulticallerOpcodesPayload::Opcodes(payload) => {
                trace!("uniswap v3 transfer token={:?}, to={:?}, amount={:?}", token_from_address, flash_pool.get_address(), amount_in);
                let mut payload = payload;
                let mut transfer_opcode = MulticallerCall::new_call(
                    token_from_address,
                    &AbiEncoderHelper::encode_erc20_transfer(flash_pool.get_address(), amount_in.unwrap_or_default()),
                );

                if amount_in.is_not_set() {
                    transfer_opcode.set_call_stack(false, 1, 0x24, 0x20);
                }

                payload.add(transfer_opcode);

                MulticallerOpcodesPayload::Opcodes(payload)
            }
            _ => payload,
        };

        let inside_call_bytes = payload.encode()?;

        trace!(
            "uniswap v3 flash swap in amount for pool={:?}  from {} to {} amount={:?}",
            flash_pool.get_address(),
            token_from_address,
            token_to_address,
            amount_in,
        );
        let mut swap_opcode = MulticallerCall::new_call(
            flash_pool.get_address(),
            &abi_encoder.encode_swap_in_amount_provided(
                flash_pool,
                token_from_address,
                token_to_address,
                amount_in.unwrap_or_default(),
                swap_to,
                inside_call_bytes,
            )?,
        );

        if amount_in.is_not_set() {
            swap_opcode.set_call_stack(
                false,
                0,
                abi_encoder.swap_in_amount_offset(flash_pool, token_from_address, token_to_address).unwrap(),
                0x20,
            );
        }

        swap_opcodes.add(swap_opcode);

        Ok(())
    }

    fn encode_flash_swap_out_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_out: SwapAmountType,
        flash_pool: &dyn Pool,
        next_pool: Option<&dyn Pool>,
        payload: MulticallerOpcodesPayload,
        multicaller_address: Address,
    ) -> eyre::Result<()> {
        let swap_to = next_pool.and_then(|next_pool| next_pool.preswap_requirement().address()).unwrap_or(multicaller_address);

        let payload = if let MulticallerOpcodesPayload::Opcodes(inside_opcodes) = payload {
            let mut inside_opcodes = inside_opcodes;
            //if next_pool.is_none() {
            trace!("retflash transfer token={:?}, to={:?}, amount=stack_norel_1", token_from_address, flash_pool.get_address());
            let mut transfer_opcode = MulticallerCall::new_call(
                token_from_address,
                &AbiEncoderHelper::encode_erc20_transfer(flash_pool.get_address(), U256::ZERO),
            );
            transfer_opcode.set_call_stack(false, 1, 0x24, 0x20);

            inside_opcodes.add(transfer_opcode);

            MulticallerOpcodesPayload::Opcodes(inside_opcodes)
        } else {
            payload
        };

        let inside_call_bytes = payload.encode()?;

        trace!(
            "uniswap v3 swap out amount provided for pool={:?}, from={} to={} amount_out={:?} receiver={} inside_opcodes_len={}",
            flash_pool.get_address(),
            token_from_address,
            token_to_address,
            amount_out,
            swap_to,
            inside_call_bytes.len()
        );

        let mut swap_opcode = MulticallerCall::new_call(
            flash_pool.get_address(),
            &abi_encoder.encode_swap_out_amount_provided(
                flash_pool,
                token_from_address,
                token_to_address,
                amount_out.unwrap_or_default(),
                swap_to,
                inside_call_bytes,
            )?,
        );

        if amount_out.is_not_set() {
            trace!("uniswap v3 swap out amount is not set");

            swap_opcode.set_call_stack(
                true,
                0,
                abi_encoder.swap_out_amount_offset(flash_pool, token_from_address, token_to_address).unwrap(),
                0x20,
            );

            swap_opcodes.add(MulticallerCall::new_calculation_call(&Bytes::from(vec![0x2, 0x2A, 0x00])));
        };

        swap_opcodes.add(swap_opcode);

        Ok(())
    }
}
