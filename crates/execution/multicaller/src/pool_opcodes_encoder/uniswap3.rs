use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::SwapOpcodesEncoderTrait;
use crate::AbiEncoderHelper;
use alloy_primitives::{Address, Bytes, U256};
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
        multicaller_address: Address,
    ) -> eyre::Result<()> {
        let inside_call_payload = Bytes::from(token_from_address.to_vec());

        let swap_to: Address = if let Some(next_pool) = next_pool {
            match abi_encoder.preswap_requirement(next_pool) {
                PreswapRequirement::Transfer(next_funds_to) => next_funds_to,
                _ => multicaller_address,
            }
        } else {
            multicaller_address
        };

        let mut swap_opcode = match amount_in {
            SwapAmountType::Set(amount) => {
                trace!("uniswap v3 i == 0 set amount in for pool={:?}, amount={}", cur_pool.get_address(), amount);
                MulticallerCall::new_call(
                    cur_pool.get_address(),
                    &abi_encoder.encode_swap_in_amount_provided(
                        cur_pool,
                        token_from_address,
                        token_to_address,
                        amount,
                        swap_to,
                        inside_call_payload,
                    )?,
                )
            }
            SwapAmountType::Balance(addr) => {
                trace!("uniswap v3 i == 0 balance of for pool={:?}, addr={}", cur_pool.get_address(), addr);
                let mut balance_opcode =
                    MulticallerCall::new_static_call(token_from_address, &AbiEncoderHelper::encode_erc20_balance_of(addr));
                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);

                swap_opcodes.add(balance_opcode);

                let mut swap_opcode = MulticallerCall::new_call(
                    cur_pool.get_address(),
                    &abi_encoder.encode_swap_in_amount_provided(
                        cur_pool,
                        token_from_address,
                        token_to_address,
                        U256::ZERO,
                        swap_to,
                        inside_call_payload,
                    )?,
                );

                swap_opcode.set_call_stack(
                    true,
                    0,
                    abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                    0x20,
                );
                swap_opcode
            }
            _ => {
                trace!("uniswap v3 i == 0 else for pool={:?}", cur_pool.get_address());
                let mut swap_opcode = MulticallerCall::new_call(
                    cur_pool.get_address(),
                    &abi_encoder.encode_swap_in_amount_provided(
                        cur_pool,
                        token_from_address,
                        token_to_address,
                        U256::ZERO,
                        swap_to,
                        inside_call_payload,
                    )?,
                );

                swap_opcode.set_call_stack(
                    true,
                    0,
                    abi_encoder.swap_in_amount_offset(cur_pool, token_from_address, token_to_address).unwrap(),
                    0x20,
                );
                swap_opcode
            }
        };

        swap_opcode.set_return_stack(
            true,
            0,
            abi_encoder.swap_in_amount_return_offset(cur_pool, token_from_address, token_to_address).unwrap(),
            0x20,
        );

        swap_opcodes.add(swap_opcode);

        if next_pool.is_some() {
            trace!("has next pool");
            if let Some(x) = abi_encoder.swap_in_amount_return_script(cur_pool, token_from_address, token_to_address) {
                let calc_opcode = MulticallerCall::new_calculation_call(&x);
                swap_opcodes.add(calc_opcode);
            }
        }
        Ok(())
    }
}
