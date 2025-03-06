use crate::opcodes_helpers::OpcodesHelpers;
use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::swap_opcodes_encoders::MulticallerOpcodesPayload;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use alloy_primitives::{Address, Bytes};
use eyre::{eyre, Result};
use loom_defi_abi::AbiEncoderHelper;
use loom_defi_address_book::TokenAddressEth;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{Pool, SwapAmountType};

pub struct StEthSwapEncoder();

impl SwapOpcodesEncoderTrait for StEthSwapEncoder {
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
        _payload: MulticallerOpcodesPayload,
        multicaller: Address,
    ) -> Result<()> {
        let pool_address = cur_pool.get_address();

        if token_from_address == TokenAddressEth::WETH && token_to_address == TokenAddressEth::STETH {
            let weth_withdraw_opcode =
                MulticallerCall::new_call(token_from_address, &AbiEncoderHelper::encode_weth_withdraw(amount_in.unwrap_or_default()));
            let swap_opcode = MulticallerCall::new_call_with_value(
                pool_address,
                &abi_encoder.encode_swap_in_amount_provided(
                    cur_pool,
                    token_from_address,
                    token_to_address,
                    amount_in.unwrap_or_default(),
                    multicaller,
                    Bytes::new(),
                )?,
                amount_in.unwrap_or_default(),
            );

            let opcodes_vec = vec![(weth_withdraw_opcode, 0x4, 0x20), (swap_opcode, 0x0, 0)];

            swap_opcodes.merge(OpcodesHelpers::build_multiple_stack(amount_in, opcodes_vec, Some(token_from_address))?);

            if next_pool.is_some() {
                let mut steth_balance_opcode =
                    MulticallerCall::new_static_call(token_to_address, &AbiEncoderHelper::encode_erc20_balance_of(multicaller));
                steth_balance_opcode.set_return_stack(true, 0, 0, 0x20);
                swap_opcodes.add(steth_balance_opcode);
            }

            return Ok(());
        }

        Err(eyre!("CANNOT_ENCODE_STETH_SWAP"))
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
