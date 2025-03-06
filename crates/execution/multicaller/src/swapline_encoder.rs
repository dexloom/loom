use std::sync::Arc;

use alloy_primitives::{Address, U256};
use eyre::{eyre, Result};
use tracing::trace;

use crate::pool_abi_encoder::ProtocolAbiSwapEncoderTrait;
use crate::pool_opcodes_encoder::{MulticallerOpcodesPayload, ProtocolSwapOpcodesEncoderV2, SwapOpcodesEncoderTrait};
use crate::ProtocolABIEncoderV2;
use loom_defi_abi::AbiEncoderHelper;
use loom_defi_address_book::TokenAddressEth;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::SwapAmountType::RelativeStack;
use loom_types_entities::{PoolWrapper, SwapAmountType, SwapLine, Token};

#[derive(Clone)]
pub struct SwapLineEncoder {
    pub multicaller_address: Address,
    abi_encoder: Arc<dyn ProtocolAbiSwapEncoderTrait>,
    opcodes_encoder: Arc<dyn SwapOpcodesEncoderTrait>,
}

impl SwapLineEncoder {
    pub fn new(
        multicaller_address: Address,
        abi_encoder: Arc<dyn ProtocolAbiSwapEncoderTrait>,
        opcodes_encoder: Arc<dyn SwapOpcodesEncoderTrait>,
    ) -> SwapLineEncoder {
        SwapLineEncoder { multicaller_address, abi_encoder, opcodes_encoder }
    }

    pub fn default_with_address(multicaller_address: Address) -> SwapLineEncoder {
        let abi_encoder = Arc::new(ProtocolABIEncoderV2::default());
        let opcodes_encoder = Arc::new(ProtocolSwapOpcodesEncoderV2::default());

        SwapLineEncoder { multicaller_address, abi_encoder, opcodes_encoder }
    }

    pub fn encode_flash_swap_line_in_amount(
        &self,
        swap_path: &SwapLine<LoomDataTypesEthereum>,
        inside_swap_opcodes: MulticallerCalls,
        funds_to: Option<&PoolWrapper>,
    ) -> Result<MulticallerCalls> {
        trace!("encode_flash_swap_line_in_amount funds_to={:?}", funds_to);
        let mut flash_swap_opcodes = MulticallerCalls::new();
        let mut inside_opcodes = inside_swap_opcodes.clone();

        let mut reverse_pools: Vec<PoolWrapper> = swap_path.pools().clone();
        reverse_pools.reverse();
        let mut reverse_tokens: Vec<Arc<Token>> = swap_path.tokens().clone();
        reverse_tokens.reverse();

        let mut prev_pool: Option<&PoolWrapper> = funds_to;

        for (pool_idx, flash_pool) in reverse_pools.iter().enumerate() {
            let token_from_address = reverse_tokens[pool_idx + 1].get_address();
            let token_to_address = reverse_tokens[pool_idx].get_address();

            let amount_in = if pool_idx == swap_path.pools().len() - 1 { swap_path.amount_in } else { SwapAmountType::RelativeStack(0) };

            self.opcodes_encoder.encode_flash_swap_in_amount_provided(
                &mut flash_swap_opcodes,
                self.abi_encoder.as_ref(),
                token_from_address,
                token_to_address,
                amount_in,
                flash_pool.as_ref(),
                prev_pool.map(|v| v.as_ref()),
                MulticallerOpcodesPayload::Opcodes(inside_opcodes),
                self.multicaller_address,
            )?;

            prev_pool = Some(flash_pool);
            inside_opcodes = flash_swap_opcodes.clone();
        }

        Ok(flash_swap_opcodes)
    }

    pub fn encode_flash_swap_line_out_amount(
        &self,
        swap_path: &SwapLine<LoomDataTypesEthereum>,
        inside_swap_opcodes: MulticallerCalls,
    ) -> Result<MulticallerCalls> {
        trace!("encode_flash_swap_line_out_amount inside_opcodes={}", inside_swap_opcodes.len());
        let mut flash_swap_opcodes = MulticallerCalls::new();
        let mut inside_opcodes = inside_swap_opcodes.clone();

        let pools: Vec<PoolWrapper> = swap_path.pools().clone();

        let tokens: Vec<Arc<Token>> = swap_path.tokens().clone();

        for (pool_idx, flash_pool) in pools.iter().enumerate() {
            flash_swap_opcodes = MulticallerCalls::new();

            let token_from_address = tokens[pool_idx].get_address();
            let token_to_address = tokens[pool_idx + 1].get_address();

            let next_pool = if pool_idx < pools.len() - 1 { Some(&pools[pool_idx + 1]) } else { None };

            let amount_out = if pool_idx == pools.len() - 1 { swap_path.amount_out } else { SwapAmountType::RelativeStack(0) };

            self.opcodes_encoder.encode_flash_swap_out_amount_provided(
                &mut flash_swap_opcodes,
                self.abi_encoder.as_ref(),
                token_from_address,
                token_to_address,
                amount_out,
                flash_pool.as_ref(),
                next_pool.map(|v| v.as_ref()),
                MulticallerOpcodesPayload::Opcodes(inside_opcodes),
                self.multicaller_address,
            )?;

            inside_opcodes = flash_swap_opcodes.clone();

            /*let swap_to = match next_pool {
                Some(next_pool) => next_pool.get_address(),
                None => self.multicaller_address,
            };

            match flash_pool.get_class() {
                PoolClass::UniswapV2 => {
                    let mut get_in_amount_opcode =
                        MulticallerCall::new_internal_call(&AbiEncoderHelper::encode_multicaller_uni2_get_in_amount(
                            token_from_address,
                            token_to_address,
                            flash_pool.get_address(),
                            amount_out.unwrap_or_default(),
                            flash_pool.get_fee(),
                        ));

                    if amount_out.is_not_set() {
                        get_in_amount_opcode.set_call_stack(false, 0, 0x24, 0x20);
                    }

                    inside_opcodes.insert(get_in_amount_opcode);

                    if pool_idx == 0 && swap_to != flash_pool.get_address() {
                        trace!(
                            "retflash transfer token={:?}, to={:?}, amount=stack_no_rel_1",
                            token_from_address,
                            flash_pool.get_address()
                        );

                        let mut transfer_opcode = MulticallerCall::new_call(
                            token_from_address,
                            &AbiEncoderHelper::encode_erc20_transfer(flash_pool.get_address(), U256::ZERO),
                        );
                        transfer_opcode.set_call_stack(false, 1, 0x24, 0x20);
                        inside_opcodes.add(transfer_opcode);
                    };

                    if swap_to != self.multicaller_address {
                        trace!("retflash transfer token={:?}, to={:?}, amount=stack_norel_0", token_to_address, swap_to);

                        let mut transfer_opcode =
                            MulticallerCall::new_call(token_to_address, &AbiEncoderHelper::encode_erc20_transfer(swap_to, U256::ZERO));
                        transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                        inside_opcodes.add(transfer_opcode);
                    }
                }
                PoolClass::UniswapV3 | PoolClass::Maverick | PoolClass::PancakeV3 => {
                    if pool_idx == 0 {
                        trace!("retflash transfer token={:?}, to={:?}, amount=stack_norel_1", token_from_address, flash_pool.get_address());
                        let mut transfer_opcode = MulticallerCall::new_call(
                            token_from_address,
                            &AbiEncoderHelper::encode_erc20_transfer(flash_pool.get_address(), U256::ZERO),
                        );
                        transfer_opcode.set_call_stack(false, 1, 0x24, 0x20);

                        inside_opcodes.add(transfer_opcode);
                    }
                }

                _ => {
                    return Err(eyre!("CANNOT_ENCODE_FLASH_CALL"));
                }
            }

            let inside_call_bytes = OpcodesEncoderV2::pack_do_calls_data(&inside_opcodes)?;
            flash_swap_opcodes = MulticallerCalls::new();

            trace!("flash swap_to {:?}", swap_to);


             */
            /*match flash_pool.get_class() {
                PoolClass::UniswapV2 => {
                    trace!(
                        "uniswap v2 swap out amount provided for pool={:?}, amount_out={:?} receiver={} inside_opcodes_len={}",
                        flash_pool.get_address(),
                        amount_out,
                        self.multicaller_address,
                        inside_opcodes.len()
                    );
                    let mut swap_opcode = MulticallerCall::new_call(
                        flash_pool.get_address(),
                        &self.abi_encoder.encode_swap_out_amount_provided(
                            flash_pool.as_ref(),
                            token_from_address,
                            token_to_address,
                            amount_out.unwrap_or_default(),
                            self.multicaller_address,
                            inside_call_bytes,
                        )?,
                    );

                    if amount_out.is_not_set() {
                        trace!("uniswap v2 amount not set");
                        swap_opcode.set_call_stack(
                            true,
                            0,
                            self.abi_encoder.swap_out_amount_offset(flash_pool.as_ref(), token_from_address, token_to_address).unwrap(),
                            0x20,
                        );
                    }

                    flash_swap_opcodes.add(swap_opcode);

                    inside_opcodes = flash_swap_opcodes.clone();
                }
                PoolClass::UniswapV3 | PoolClass::PancakeV3 | PoolClass::Maverick => {
                    trace!(
                        "uniswap v3 swap out amount provided for pool={:?}, amount_out={:?} receiver={} inside_opcodes_len={}",
                        flash_pool.get_address(),
                        amount_out,
                        swap_to,
                        inside_opcodes.len()
                    );
                    let mut swap_opcode = MulticallerCall::new_call(
                        flash_pool.get_address(),
                        &self.abi_encoder.encode_swap_out_amount_provided(
                            flash_pool.as_ref(),
                            token_from_address,
                            token_to_address,
                            amount_out.unwrap_or_default(),
                            swap_to,
                            inside_call_bytes,
                        )?,
                    );

                    if amount_out.is_not_set() {
                        trace!("uniswap v3 swap out amount is not set");

                        flash_swap_opcodes.add(MulticallerCall::new_calculation_call(&Bytes::from(vec![0x8, 0x2A, 0x00])));

                        swap_opcode.set_call_stack(
                            true,
                            0,
                            self.abi_encoder.swap_out_amount_offset(flash_pool.as_ref(), token_from_address, token_to_address).unwrap(),
                            0x20,
                        );
                    };

                    flash_swap_opcodes.add(swap_opcode);

                    inside_opcodes = flash_swap_opcodes.clone();
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_FLASH_CALL"));
                }
            }

             */
        }

        Ok(flash_swap_opcodes)
    }

    pub fn encode_flash_swap_dydx(&self, _inside_swap_opcodes: MulticallerCalls, _funds_from: Address) -> Result<MulticallerCalls> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    pub fn encode_swap_line_in_amount(
        &self,
        swap_path: &SwapLine<LoomDataTypesEthereum>,
        funds_to: Option<&PoolWrapper>,
    ) -> Result<MulticallerCalls> {
        let mut swap_opcodes = MulticallerCalls::new();

        let mut amount_in = swap_path.amount_in;

        for i in 0..swap_path.pools().len() {
            let token_from_address = swap_path.tokens()[i].get_address();
            let token_to_address = swap_path.tokens()[i + 1].get_address();

            let cur_pool = &swap_path.pools()[i].clone();
            let next_pool: Option<&PoolWrapper> = if i < swap_path.pools().len() - 1 { Some(&swap_path.pools()[i + 1]) } else { funds_to };

            trace!(
                "encode_swap_line_in_amount for from={} to={} pool={}, next_pool={:?} funds_to {:?}",
                token_from_address,
                token_to_address,
                cur_pool.get_address(),
                next_pool.map(|next_pool| next_pool.get_address()),
                funds_to
            );

            /*let amount_in = if i == 0 {
                /*if let PreswapRequirement::Transfer(funds_needed_at) = self.abi_encoder.preswap_requirement(cur_pool.as_ref()) {
                    if funds_needed_at != funds_from {
                        trace!(
                            "encode_swap_line_in_amount  i == 0  amount in {:?} funds {}->{}",
                            swap_path.amount_in,
                            funds_from,
                            funds_needed_at
                        );
                        match swap_path.amount_in {
                            SwapAmountType::Set(value) => {
                                trace!("transfer token={:?}, to={:?}, amount={}", token_from_address, funds_needed_at, value);
                                let transfer_opcode = MulticallerCall::new_call(
                                    token_from_address,
                                    &AbiEncoderHelper::encode_erc20_transfer(funds_needed_at, value),
                                );
                                swap_opcodes.add(transfer_opcode);
                                swap_path.amount_in
                            }
                            SwapAmountType::Balance(addr) => {
                                trace!("encode_swap_line_in_amount  i == 0 balance of addr={:?}", addr);
                                let mut balance_opcode =
                                    MulticallerCall::new_static_call(token_from_address, &AbiEncoderHelper::encode_erc20_balance_of(addr));
                                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
                                swap_opcodes.add(balance_opcode);

                                trace!("transfer token={:?}, to={:?}, amount=from stack", token_from_address, cur_pool.get_address());
                                let mut transfer_opcode = MulticallerCall::new_call(
                                    token_from_address,
                                    &AbiEncoderHelper::encode_erc20_transfer(funds_needed_at, U256::ZERO),
                                );
                                transfer_opcode.set_call_stack(true, 0, 0x24, 0x20);
                                swap_opcodes.add(transfer_opcode);
                                SwapAmountType::RelativeStack(0)
                            }
                            _ => {
                                trace!("encode_swap_line_in_amount i == 0");
                                trace!("transfer token={:?}, to={:?}, amount=from stack", token_from_address, cur_pool.get_address());
                                let mut transfer_opcode = MulticallerCall::new_call(
                                    token_from_address,
                                    &AbiEncoderHelper::encode_erc20_transfer(funds_needed_at, U256::ZERO),
                                );
                                transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                                swap_opcodes.add(transfer_opcode);
                                swap_path.amount_in
                            }
                        }
                    } else {
                        swap_path.amount_in
                    }
                } else {
                    swap_path.amount_in
                }

                     */
                swap_path.amount_in
            } else {
                SwapAmountType::RelativeStack(0)
            };

             */

            /*let swap_to: Address = if let Some(next_pool) = next_pool {
                match &self.abi_encoder.preswap_requirement(next_pool.as_ref()) {
                    PreswapRequirement::Transfer(next_funds_to) => *next_funds_to,
                    _ => self.multicaller_address,
                }
            } else {
                funds_to
            };*/

            let swap_to = next_pool.map(|x| x.get_address()).unwrap_or(self.multicaller_address);

            trace!("swap_to {:?}", swap_to);

            self.opcodes_encoder.encode_swap_in_amount_provided(
                &mut swap_opcodes,
                self.abi_encoder.as_ref(),
                token_from_address,
                token_to_address,
                amount_in,
                cur_pool.as_ref(),
                next_pool.map(|next_pool| next_pool.as_ref()),
                MulticallerOpcodesPayload::Empty,
                self.multicaller_address,
            )?;

            amount_in = RelativeStack(0);
        }
        Ok(swap_opcodes)
    }

    pub fn encode_tips(
        &self,
        swap_opcodes: MulticallerCalls,
        token_address: Address,
        min_balance: U256,
        tips: U256,
        to: Address,
    ) -> Result<MulticallerCalls> {
        let mut tips_opcodes = swap_opcodes.clone();

        let call_data = if token_address == TokenAddressEth::WETH {
            trace!("encode_multicaller_transfer_tips_weth");
            AbiEncoderHelper::encode_multicaller_transfer_tips_weth(min_balance, tips, to)
        } else {
            trace!("encode_multicaller_transfer_tips");
            AbiEncoderHelper::encode_multicaller_transfer_tips(token_address, min_balance, tips, to)
        };
        tips_opcodes.add(MulticallerCall::new_internal_call(&call_data));
        Ok(tips_opcodes)
    }
}
