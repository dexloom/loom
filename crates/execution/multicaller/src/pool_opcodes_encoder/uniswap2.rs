use crate::opcodes_helpers::OpcodesHelpers;
use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use alloy_primitives::{Address, Bytes, U256};
use eyre::eyre;
use loom_defi_abi::AbiEncoderHelper;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{Pool, PreswapRequirement, SwapAmountType};
use tracing::{trace, warn};

pub struct UniswapV2SwapOpcodesEncoder;

impl SwapOpcodesEncoderTrait for UniswapV2SwapOpcodesEncoder {
    fn encode_swap_in_amount_provided(
        &self,
        swap_opcodes: &mut MulticallerCalls,
        abi_encoder: &dyn ProtocolAbiSwapEncoderTrait,
        token_from_address: Address,
        token_to_address: Address,
        amount_in: SwapAmountType,
        cur_pool: &dyn Pool,
        next_pool: Option<&dyn Pool>,
        _payload: MulticallerOpcodesPayload,
        multicaller_address: Address,
    ) -> eyre::Result<()> {
        // Getting destination address
        let swap_to = next_pool.and_then(|next_pool| next_pool.preswap_requirement().address()).unwrap_or(multicaller_address);

        trace!(
            "uniswap v2 get out amount for pool={:?}, amount={:?} from {} to {}",
            cur_pool.get_address(),
            amount_in,
            token_from_address,
            token_to_address
        );

        // calculating out amount for in amount provided
        let get_out_amount_opcode = MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_out_amount(
            token_from_address,
            token_to_address,
            cur_pool.get_address(),
            amount_in.unwrap_or_default(),
            cur_pool.get_fee(),
        ));

        // setting argument from stack if it is required
        swap_opcodes.merge(OpcodesHelpers::build_call_stack(amount_in, get_out_amount_opcode, 0x24, 0x20, Some(token_from_address))?);

        // abi encode and add uniswap swap opcode
        let mut swap_opcode = MulticallerCall::new_call(
            cur_pool.get_address(),
            &abi_encoder.encode_swap_out_amount_provided(
                cur_pool,
                token_from_address,
                token_to_address,
                U256::from(1),
                swap_to,
                Bytes::new(),
            )?,
        );

        // setting stack swap argument based on calculated out amount
        swap_opcode.set_call_stack(
            true,
            0,
            abi_encoder.swap_out_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
            0x20,
        );

        swap_opcodes.add(swap_opcode);

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
        let payload = if let MulticallerOpcodesPayload::Opcodes(inside_opcodes) = &payload {
            let mut inside_opcodes = inside_opcodes.clone();

            match amount_in {
                SwapAmountType::Set(amount) => {
                    // first swap, need to return token_from to this pool, otherwise tokens should be already on the contract
                    trace!("uniswap v2 transfer token={:?}, to={:?}, amount={}", token_from_address, flash_pool.get_address(), amount);

                    inside_opcodes.add(MulticallerCall::new_call(
                        token_from_address,
                        &AbiEncoderHelper::encode_erc20_transfer(flash_pool.get_address(), amount),
                    ));
                }
                _ => {
                    warn!("InAmountTypeNotSet");
                }
            }

            // if there is a prev_pool transfer funds in case it is uniswap2.
            if let Some(prev_pool) = prev_pool {
                if let PreswapRequirement::Transfer(swap_to) = prev_pool.preswap_requirement() {
                    trace!("uniswap v2 transfer token_to_address={:?}, funds_to={:?} amount=stack_norel_0", token_to_address, swap_to);
                    let mut transfer_opcode =
                        MulticallerCall::new_call(token_to_address, &AbiEncoderHelper::encode_erc20_transfer(swap_to, U256::ZERO));
                    transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                    inside_opcodes.insert(transfer_opcode);
                }
            }
            MulticallerOpcodesPayload::Opcodes(inside_opcodes)
        } else {
            payload
        };

        let inside_call_bytes = payload.encode()?;

        // getting out amount for in amount provided

        trace!("uniswap v2 get out amount for pool={:?}, amount={:?}", flash_pool.get_address(), amount_in);
        let mut get_out_amount_opcode = MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_out_amount(
            token_from_address,
            token_to_address,
            flash_pool.get_address(),
            amount_in.unwrap_or_default(),
            flash_pool.get_fee(),
        ));

        // setting up stack, in amount is out amount for previous swap and is located in stack0
        if amount_in.is_not_set() {
            get_out_amount_opcode.set_call_stack(false, 0, 0x24, 0x20);
        }

        // abi encode uniswap2 out amount provided swap.
        let mut swap_opcode = MulticallerCall::new_call(
            flash_pool.get_address(),
            &abi_encoder.encode_swap_out_amount_provided(
                flash_pool,
                token_from_address,
                token_to_address,
                U256::ZERO,
                multicaller_address,
                inside_call_bytes,
            )?,
        );

        // setting call stack to calculated out amount.
        swap_opcode.set_call_stack(
            true,
            0,
            abi_encoder.swap_out_amount_offset(flash_pool, token_from_address, token_to_address).unwrap(),
            0x20,
        );

        swap_opcodes.add(get_out_amount_opcode).add(swap_opcode);

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
        // getting address for token_to
        let swap_to = next_pool.and_then(|next_pool| next_pool.preswap_requirement().address()).unwrap_or(multicaller_address);

        // add get_in amount to keep amount we should return in stack.
        let payload = if let MulticallerOpcodesPayload::Opcodes(inside_opcodes) = &payload {
            let mut inside_opcodes = inside_opcodes.clone();

            let mut get_in_amount_opcode = MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_in_amount(
                token_from_address,
                token_to_address,
                flash_pool.get_address(),
                amount_out.unwrap_or_default(),
                flash_pool.get_fee(),
            ));

            // is not set, will use stack0
            if amount_out.is_not_set() {
                get_in_amount_opcode.set_call_stack(false, 0, 0x24, 0x20);
            }

            inside_opcodes.insert(get_in_amount_opcode);

            // if we need funds somewhere else, we transfer it because cannot set destination in uniswap2 with parameters
            if swap_to != multicaller_address {
                trace!("retflash transfer token={:?}, to={:?}, amount=stack_norel_0", token_to_address, swap_to);

                let mut transfer_opcode =
                    MulticallerCall::new_call(token_to_address, &AbiEncoderHelper::encode_erc20_transfer(swap_to, U256::ZERO));
                transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                inside_opcodes.add(transfer_opcode);
            }

            MulticallerOpcodesPayload::Opcodes(inside_opcodes)
        } else {
            payload
        };

        // encode inside bytes
        let inside_call_bytes = payload.encode()?;

        // flash swap without amount
        trace!(
            "uniswap v2 swap out amount provided for pool={:?}, from={} to={} amount_out={:?} receiver={} inside_opcodes_len={}",
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
                multicaller_address,
                inside_call_bytes,
            )?,
        );

        // set rel_stack(0) is amount is not set
        if amount_out.is_not_set() {
            trace!("uniswap v2 amount not set");
            swap_opcode.set_call_stack(
                true,
                0,
                abi_encoder.swap_out_amount_offset(flash_pool, token_from_address, token_to_address).unwrap(),
                0x20,
            );
        }

        swap_opcodes.add(swap_opcode);
        Ok(())
    }
}
