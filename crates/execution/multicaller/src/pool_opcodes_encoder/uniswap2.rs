use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use crate::AbiEncoderHelper;
use alloy_primitives::{Address, Bytes, U256};
use eyre::eyre;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{Pool, PreswapRequirement, SwapAmountType};
use tracing::trace;

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
        let swap_to: Address = if let Some(next_pool) = next_pool {
            match abi_encoder.preswap_requirement(next_pool) {
                PreswapRequirement::Transfer(next_funds_to) => next_funds_to,
                _ => multicaller_address,
            }
        } else {
            multicaller_address
        };

        match amount_in {
            SwapAmountType::Set(value) => {
                trace!("uniswap v2 i == 0 set amount in {}", value);

                let get_out_amount_opcode = MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_out_amount(
                    token_from_address,
                    token_to_address,
                    cur_pool.get_address(),
                    value,
                    cur_pool.get_fee(),
                ));
                swap_opcodes.add(get_out_amount_opcode);
            }
            SwapAmountType::Balance(addr) => {
                trace!("uniswap v2 i == 0 balance of addr={:?}", addr);
                let mut balance_opcode =
                    MulticallerCall::new_static_call(token_from_address, &AbiEncoderHelper::encode_erc20_balance_of(addr));
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
                swap_opcodes.add(balance_opcode);

                trace!("uni2 get out amount from={:?}, to={:?}, pool={:?}", token_from_address, token_to_address, cur_pool.get_address());
                let mut get_out_amount_opcode =
                    MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_out_amount(
                        token_from_address,
                        token_to_address,
                        cur_pool.get_address(),
                        U256::ZERO,
                        cur_pool.get_fee(),
                    ));
                get_out_amount_opcode.set_call_stack(true, 0, 0x24, 0x20);
                swap_opcodes.add(get_out_amount_opcode);
            }
            _ => {
                trace!("uniswap v2");

                let mut get_out_amount_opcode =
                    MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_out_amount(
                        token_from_address,
                        token_to_address,
                        cur_pool.get_address(),
                        U256::ZERO,
                        cur_pool.get_fee(),
                    ));
                get_out_amount_opcode.set_call_stack(true, 0, 0x24, 0x20);
                swap_opcodes.add(get_out_amount_opcode);
            }
        }

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
}
