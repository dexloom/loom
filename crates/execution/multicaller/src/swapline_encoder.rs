use std::sync::Arc;

use alloy_primitives::{Address, Bytes, U256};
use eyre::{eyre, Result};
use tracing::{trace, warn};

use loom_defi_address_book::TokenAddressEth;
use loom_types_blockchain::LoomDataTypesEthereum;
use loom_types_blockchain::{MulticallerCall, MulticallerCalls};
use loom_types_entities::{PoolClass, PoolWrapper, PreswapRequirement, SwapAmountType, SwapLine, Token};

use crate::helpers::EncoderHelper;
use crate::opcodes_encoder::{OpcodesEncoder, OpcodesEncoderV2};
use crate::poolencoders::{CurveSwapEncoder, StEthSwapEncoder, WstEthSwapEncoder};

#[derive(Clone)]
pub struct SwapLineEncoder {
    multicaller: Address,
}

impl SwapLineEncoder {
    pub fn new(multicaller: Address) -> SwapLineEncoder {
        SwapLineEncoder { multicaller }
    }

    pub fn encode_flash_swap_line_in_amount(
        &self,
        swap_path: &SwapLine<LoomDataTypesEthereum>,
        inside_swap_opcodes: MulticallerCalls,
        funds_to: Address,
    ) -> Result<MulticallerCalls> {
        trace!("encode_flash_swap_line_in_amount funds_to={}", funds_to);
        let mut flash_swap_opcodes = MulticallerCalls::new();
        let mut inside_opcodes = inside_swap_opcodes.clone();

        let mut reverse_pools: Vec<PoolWrapper> = swap_path.pools().clone();
        reverse_pools.reverse();
        let mut reverse_tokens: Vec<Arc<Token>> = swap_path.tokens().clone();
        reverse_tokens.reverse();

        let mut prev_pool: Option<&PoolWrapper> = None;

        for (pool_idx, flash_pool) in reverse_pools.iter().enumerate() {
            let token_from_address = reverse_tokens[pool_idx + 1].get_address();
            let token_to_address = reverse_tokens[pool_idx].get_address();

            let amount_in = if pool_idx == swap_path.pools().len() - 1 { swap_path.amount_in } else { SwapAmountType::Stack0 };

            let swap_to = match prev_pool {
                Some(prev_pool) => match flash_pool.get_class() {
                    PoolClass::UniswapV2 => match prev_pool.get_encoder().preswap_requirement() {
                        PreswapRequirement::Transfer(transfer_to) => {
                            trace!(
                                "uniswap v2 transfer to previous pool: token={:?}, to={:?}, prev_pool={:?}",
                                token_to_address,
                                transfer_to,
                                prev_pool.get_address()
                            );
                            let mut transfer_opcode =
                                MulticallerCall::new_call(token_to_address, &EncoderHelper::encode_erc20_transfer(transfer_to, U256::ZERO));
                            transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                            inside_opcodes.insert(transfer_opcode);
                            transfer_to
                        }
                        _ => self.multicaller,
                    },
                    _ => match prev_pool.get_encoder().preswap_requirement() {
                        PreswapRequirement::Transfer(funds_to) => {
                            trace!("other transfer to previous pool: funds_to={:?}, prev_pool={:?}", funds_to, prev_pool);
                            funds_to
                        }
                        _ => {
                            trace!("other swap to multicaller, prev_pool={:?}", prev_pool);
                            self.multicaller
                        }
                    },
                },
                None => funds_to,
            };

            match flash_pool.get_class() {
                PoolClass::UniswapV2 => {
                    match amount_in {
                        SwapAmountType::Set(amount) => {
                            trace!(
                                "uniswap v2 transfer token={:?}, to={:?}, amount={}",
                                token_from_address,
                                flash_pool.get_address(),
                                amount
                            );
                            inside_opcodes.add(MulticallerCall::new_call(
                                token_from_address,
                                &EncoderHelper::encode_erc20_transfer(flash_pool.get_address(), amount),
                            ));
                        }
                        _ => {
                            warn!("InAmountTypeNotSet")
                        }
                    }

                    if pool_idx == 0 && funds_to != self.multicaller {
                        trace!("uniswap v2 transfer to token_to_address={:?}, funds_to={:?}", token_to_address, funds_to);
                        let mut transfer_opcode =
                            MulticallerCall::new_call(token_to_address, &EncoderHelper::encode_erc20_transfer(funds_to, U256::ZERO));
                        transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                        inside_opcodes.insert(transfer_opcode);
                    }
                }
                PoolClass::UniswapV3 => {
                    let transfer_opcode = match amount_in {
                        SwapAmountType::Set(amount) => {
                            trace!(
                                "uniswap v3 transfer token={:?}, to={:?}, amount={:?}",
                                token_to_address,
                                flash_pool.get_address(),
                                amount
                            );

                            MulticallerCall::new_call(
                                token_from_address,
                                &EncoderHelper::encode_erc20_transfer(flash_pool.get_address(), amount),
                            )
                        }
                        _ => {
                            trace!("other transfer token={:?}, to={:?}", token_to_address, flash_pool.get_address());
                            MulticallerCall::new_call(
                                token_from_address,
                                &EncoderHelper::encode_erc20_transfer(flash_pool.get_address(), U256::ZERO),
                            )
                            .set_call_stack(false, 1, 0x24, 0x20)
                            .clone()
                        }
                    };

                    inside_opcodes.add(transfer_opcode);
                }

                _ => {
                    return Err(eyre!("CANNOT_ENCODE_FLASH_CALL"));
                }
            }

            let inside_call_bytes = OpcodesEncoderV2::pack_do_calls_data(&inside_opcodes)?;
            flash_swap_opcodes = MulticallerCalls::new();

            trace!("flash swap_to {:?}", swap_to);

            match flash_pool.get_class() {
                PoolClass::UniswapV2 => {
                    let get_out_amount_opcode = match amount_in {
                        SwapAmountType::Set(amount) => {
                            trace!("uniswap v2 get out amount for pool={:?}, amount={}", flash_pool.get_address(), amount);
                            MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_out_amount(
                                token_from_address,
                                token_to_address,
                                flash_pool.get_address(),
                                amount,
                                flash_pool.get_fee(),
                            ))
                        }
                        _ => {
                            trace!("uniswap v2 get out amount, pool={:?}", flash_pool.get_address());
                            MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_out_amount(
                                token_from_address,
                                token_to_address,
                                flash_pool.get_address(),
                                U256::ZERO,
                                flash_pool.get_fee(),
                            ))
                            .set_call_stack(false, 0, 0x24, 0x20)
                            .clone()
                        }
                    };

                    let mut swap_opcode = MulticallerCall::new_call(
                        flash_pool.get_address(),
                        &flash_pool.get_encoder().encode_swap_out_amount_provided(
                            token_from_address,
                            token_to_address,
                            U256::ZERO,
                            self.multicaller,
                            inside_call_bytes,
                        )?,
                    );
                    swap_opcode.set_call_stack(
                        true,
                        0,
                        flash_pool.get_encoder().swap_out_amount_offset(token_from_address, token_to_address).unwrap(),
                        0x20,
                    );

                    flash_swap_opcodes.add(get_out_amount_opcode).add(swap_opcode);

                    prev_pool = Some(flash_pool);
                    inside_opcodes = flash_swap_opcodes.clone();
                }
                PoolClass::UniswapV3 => {
                    let swap_opcode = match amount_in {
                        SwapAmountType::Set(amount) => {
                            trace!("uniswap v3 swap in amount for pool={:?}, amount={}", flash_pool.get_address(), amount);
                            MulticallerCall::new_call(
                                flash_pool.get_address(),
                                &flash_pool.get_encoder().encode_swap_in_amount_provided(
                                    token_from_address,
                                    token_to_address,
                                    amount,
                                    swap_to,
                                    inside_call_bytes,
                                )?,
                            )
                        }
                        _ => {
                            trace!("uniswap v3 swap in amount for pool={:?}", flash_pool.get_address());
                            MulticallerCall::new_call(
                                flash_pool.get_address(),
                                &flash_pool.get_encoder().encode_swap_in_amount_provided(
                                    token_from_address,
                                    token_to_address,
                                    U256::ZERO,
                                    swap_to,
                                    inside_call_bytes,
                                )?,
                            )
                            .set_call_stack(
                                false,
                                0,
                                flash_pool.get_encoder().swap_in_amount_offset(token_from_address, token_to_address).unwrap(),
                                0x20,
                            )
                            .clone()
                        }
                    };

                    flash_swap_opcodes.add(swap_opcode);

                    prev_pool = Some(flash_pool);
                    inside_opcodes = flash_swap_opcodes.clone();
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_FLASH_CALL"));
                }
            }
        }

        Ok(flash_swap_opcodes)
    }

    pub fn encode_flash_swap_line_out_amount(
        &self,
        swap_path: &SwapLine<LoomDataTypesEthereum>,
        inside_swap_opcodes: MulticallerCalls,
        _funds_from: Address,
    ) -> Result<MulticallerCalls> {
        trace!("encode_flash_swap_line_out_amount");
        let mut flash_swap_opcodes = MulticallerCalls::new();
        let mut inside_opcodes = inside_swap_opcodes.clone();

        let pools: Vec<PoolWrapper> = swap_path.pools().clone();

        let tokens: Vec<Arc<Token>> = swap_path.tokens().clone();

        for (pool_idx, flash_pool) in pools.iter().enumerate() {
            let token_from_address = tokens[pool_idx].get_address();
            let token_to_address = tokens[pool_idx + 1].get_address();

            let next_pool = if pool_idx < pools.len() - 1 { Some(&pools[pool_idx + 1]) } else { None };

            let amount_out = if pool_idx == pools.len() - 1 { swap_path.amount_out } else { SwapAmountType::Stack0 };

            let swap_to = match next_pool {
                Some(next_pool) => next_pool.get_address(),
                None => self.multicaller,
            };

            match flash_pool.get_class() {
                PoolClass::UniswapV2 => {
                    let mut get_in_amount_opcode =
                        MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_in_amount(
                            token_from_address,
                            token_to_address,
                            flash_pool.get_address(),
                            amount_out.unwrap_or_zero(),
                            flash_pool.get_fee(),
                        ));

                    match amount_out {
                        SwapAmountType::Set(_) => {}
                        _ => {
                            get_in_amount_opcode.set_call_stack(false, 0, 0x24, 20);
                        }
                    }
                    inside_opcodes.insert(get_in_amount_opcode);

                    if pool_idx == 0 && swap_to != flash_pool.get_address() {
                        let mut transfer_opcode = MulticallerCall::new_call(
                            token_from_address,
                            &EncoderHelper::encode_erc20_transfer(flash_pool.get_address(), U256::ZERO),
                        );
                        transfer_opcode.set_call_stack(false, 1, 0x24, 0x20);

                        inside_opcodes.add(transfer_opcode);
                    };

                    if swap_to != self.multicaller {
                        let mut transfer_opcode =
                            MulticallerCall::new_call(token_to_address, &EncoderHelper::encode_erc20_transfer(swap_to, U256::ZERO));
                        transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                        inside_opcodes.add(transfer_opcode);
                    }

                    /*
                    match amount_out {
                        SwapAmountType::Set(amount) => {
                            inside_opcodes
                                .add( Opcode::new_call(token_to_address,
                                                       &EncoderHelper::encode_erc20_transfer(flash_pool.get_address(), amount) ) );
                        }
                        _=> {
                            error!("OutAmountTypeNotSet")
                        }
                    }
                     */
                    /*

                    let mut get_in_amount_opcode =
                        match amount_out {
                            InAmountType::Set(amount)=> {
                                Opcode::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_in_amount(
                                    token_from_address,
                                    token_to_address,
                                    flash_pool.get_address(),
                                    amount,
                                    flash_pool.get_fee()
                                ))
                            }
                            _=> {
                                error!("Uni2 In amount not handled");
                                Opcode::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_in_amount(
                                    token_from_address,
                                    token_to_address,
                                    flash_pool.get_address(),
                                    U256::zero(),
                                    flash_pool.get_fee()
                                )).set_call_stack(false, 0, 0x24,0x20).clone()
                            }
                        };

                     */

                    /*
                    if pool_idx == 0 && funds_to != self.multicaller {
                        let mut transfer_opcode = Opcode::new_call(token_to_address,
                                                                   &EncoderHelper::encode_erc20_transfer(funds_to, U256::zero()));
                        transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                        inside_opcodes
                            .insert( transfer_opcode);
                    }
                     */
                }
                PoolClass::UniswapV3 => {
                    if pool_idx == 0 {
                        let mut transfer_opcode = MulticallerCall::new_call(
                            token_from_address,
                            &EncoderHelper::encode_erc20_transfer(flash_pool.get_address(), U256::ZERO),
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

            match flash_pool.get_class() {
                PoolClass::UniswapV2 => {
                    trace!("uniswap v2 swap out amount provided for pool={:?}, amount_out={:?}", flash_pool.get_address(), amount_out);
                    let mut swap_opcode = MulticallerCall::new_call(
                        flash_pool.get_address(),
                        &flash_pool.get_encoder().encode_swap_out_amount_provided(
                            token_from_address,
                            token_to_address,
                            amount_out.unwrap_or_zero(),
                            self.multicaller,
                            inside_call_bytes,
                        )?,
                    );

                    match amount_out {
                        SwapAmountType::Set(_) => {
                            trace!("uniswap v2 amount out set amount");
                        }
                        _ => {
                            trace!("uniswap v2 amount out else");
                            swap_opcode.set_call_stack(
                                true,
                                0,
                                flash_pool.get_encoder().swap_out_amount_offset(token_from_address, token_to_address).unwrap(),
                                0x20,
                            );
                        }
                    };

                    flash_swap_opcodes.add(swap_opcode);

                    inside_opcodes = flash_swap_opcodes.clone();
                }
                PoolClass::UniswapV3 => {
                    let swap_opcode = match amount_out {
                        SwapAmountType::Set(amount) => {
                            trace!(
                                "uniswap v3 swap out amount provided for pool={:?}, amount_out={:?}",
                                flash_pool.get_address(),
                                amount_out
                            );
                            MulticallerCall::new_call(
                                flash_pool.get_address(),
                                &flash_pool.get_encoder().encode_swap_out_amount_provided(
                                    token_from_address,
                                    token_to_address,
                                    amount,
                                    swap_to,
                                    inside_call_bytes,
                                )?,
                            )
                        }
                        _ => {
                            trace!("uniswap v3 else swap out amount for pool={:?}", flash_pool.get_address());
                            flash_swap_opcodes.add(MulticallerCall::new_calculation_call(&Bytes::from(vec![0x8, 0x2A, 0x00])));

                            MulticallerCall::new_call(
                                flash_pool.get_address(),
                                &flash_pool.get_encoder().encode_swap_out_amount_provided(
                                    token_from_address,
                                    token_to_address,
                                    U256::ZERO,
                                    swap_to,
                                    inside_call_bytes,
                                )?,
                            )
                            .set_call_stack(
                                true,
                                0,
                                flash_pool.get_encoder().swap_out_amount_offset(token_from_address, token_to_address).unwrap(),
                                0x20,
                            )
                            .clone()
                        }
                    };

                    flash_swap_opcodes.add(swap_opcode);

                    inside_opcodes = flash_swap_opcodes.clone();
                }
                _ => {
                    return Err(eyre!("CANNOT_ENCODE_FLASH_CALL"));
                }
            }
        }

        Ok(flash_swap_opcodes)
    }

    pub fn encode_flash_swap_dydx(&self, _inside_swap_opcodes: MulticallerCalls, _funds_from: Address) -> Result<MulticallerCalls> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    pub fn encode_swap_line_in_amount(
        &self,
        swap_path: &SwapLine<LoomDataTypesEthereum>,
        funds_from: Address,
        funds_to: Address,
    ) -> Result<MulticallerCalls> {
        let mut swap_opcodes = MulticallerCalls::new();

        for i in 0..swap_path.pools().len() {
            let token_from_address = swap_path.tokens()[i].get_address();
            let token_to_address = swap_path.tokens()[i + 1].get_address();

            let cur_pool = &swap_path.pools()[i].clone();
            let next_pool: Option<&PoolWrapper> = if i < swap_path.pools().len() - 1 { Some(&swap_path.pools()[i + 1]) } else { None };

            trace!(
                "encode_swap_line_in_amount for from={} to={} pool={}, next_pool={:?}",
                token_from_address,
                token_to_address,
                cur_pool.get_address(),
                next_pool.map(|next_pool| next_pool.get_address())
            );

            let swap_to: Address = if let Some(next_pool) = next_pool {
                match &next_pool.get_encoder().preswap_requirement() {
                    PreswapRequirement::Transfer(next_funds_to) => *next_funds_to,
                    _ => self.multicaller,
                }
            } else {
                funds_to
            };

            trace!("swap_to {:?}", swap_to);

            match cur_pool.get_class() {
                PoolClass::UniswapV2 => {
                    if i == 0 {
                        match swap_path.amount_in {
                            SwapAmountType::Set(value) => {
                                trace!("uniswap v2 i == 0 set amount in {}", value);
                                if funds_from != cur_pool.get_address() {
                                    trace!("transfer token={:?}, to={:?}, amount={}", token_from_address, cur_pool.get_address(), value);
                                    let transfer_opcode = MulticallerCall::new_call(
                                        token_from_address,
                                        &EncoderHelper::encode_erc20_transfer(cur_pool.get_address(), value),
                                    );
                                    swap_opcodes.add(transfer_opcode);
                                }

                                let get_out_amount_opcode =
                                    MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_out_amount(
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
                                    MulticallerCall::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));
                                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);
                                swap_opcodes.add(balance_opcode);

                                if funds_from != cur_pool.get_address() {
                                    trace!("transfer token={:?}, to={:?}, amount=from stack", token_from_address, cur_pool.get_address());
                                    let mut transfer_opcode = MulticallerCall::new_call(
                                        token_from_address,
                                        &EncoderHelper::encode_erc20_transfer(cur_pool.get_address(), U256::ZERO),
                                    );
                                    transfer_opcode.set_call_stack(true, 0, 0x24, 0x20);
                                    swap_opcodes.add(transfer_opcode);
                                }
                                trace!(
                                    "uni2 get out amount from={:?}, to={:?}, pool={:?}",
                                    token_from_address,
                                    token_to_address,
                                    cur_pool.get_address()
                                );
                                let mut get_out_amount_opcode =
                                    MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_out_amount(
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
                                trace!("uniswap v2 i == 0");
                                if funds_from != cur_pool.get_address() {
                                    trace!("transfer token={:?}, to={:?}, amount=from stack", token_from_address, cur_pool.get_address());
                                    let mut transfer_opcode = MulticallerCall::new_call(
                                        token_from_address,
                                        &EncoderHelper::encode_erc20_transfer(cur_pool.get_address(), U256::ZERO),
                                    );
                                    transfer_opcode.set_call_stack(false, 0, 0x24, 0x20);
                                    swap_opcodes.add(transfer_opcode);
                                }

                                let mut get_out_amount_opcode =
                                    MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_out_amount(
                                        token_from_address,
                                        token_to_address,
                                        cur_pool.get_address(),
                                        U256::ZERO,
                                        cur_pool.get_fee(),
                                    ));
                                get_out_amount_opcode.set_call_stack(false, 0, 0x24, 0x20);
                                swap_opcodes.add(get_out_amount_opcode);
                            }
                        }
                    } else {
                        trace!("uniswap v2 i != 0 else pool={:?}", cur_pool.get_address());
                        let mut get_out_amount_opcode =
                            MulticallerCall::new_internal_call(&EncoderHelper::encode_multicaller_uni2_get_out_amount(
                                token_from_address,
                                token_to_address,
                                cur_pool.get_address(),
                                U256::from(1),
                                cur_pool.get_fee(),
                            ));
                        get_out_amount_opcode.set_call_stack(true, 0, 0x24, 0x20);
                        swap_opcodes.add(get_out_amount_opcode);
                    };

                    let mut swap_opcode = MulticallerCall::new_call(
                        cur_pool.get_address(),
                        &cur_pool.get_encoder().encode_swap_out_amount_provided(
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
                        cur_pool.get_encoder().swap_out_amount_offset(token_from_address, token_to_address).unwrap(),
                        0x20,
                    );

                    swap_opcodes.add(swap_opcode);
                }
                PoolClass::UniswapV3 => {
                    let inside_call_payload = Bytes::from(token_from_address.to_vec());

                    let mut swap_opcode = if i == 0 {
                        match swap_path.amount_in {
                            SwapAmountType::Set(amount) => {
                                trace!("uniswap v3 i == 0 set amount in for pool={:?}, amount={}", cur_pool.get_address(), amount);
                                MulticallerCall::new_call(
                                    cur_pool.get_address(),
                                    &cur_pool.get_encoder().encode_swap_in_amount_provided(
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
                                    MulticallerCall::new_static_call(token_from_address, &EncoderHelper::encode_erc20_balance_of(addr));
                                balance_opcode.set_return_stack(true, 0, 0x0, 0x20);

                                swap_opcodes.add(balance_opcode);

                                let mut swap_opcode = MulticallerCall::new_call(
                                    cur_pool.get_address(),
                                    &cur_pool.get_encoder().encode_swap_in_amount_provided(
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
                                    cur_pool.get_encoder().swap_in_amount_offset(token_from_address, token_to_address).unwrap(),
                                    0x20,
                                );
                                swap_opcode
                            }
                            _ => {
                                trace!("uniswap v3 i == 0 else for pool={:?}", cur_pool.get_address());
                                let mut swap_opcode = MulticallerCall::new_call(
                                    cur_pool.get_address(),
                                    &cur_pool.get_encoder().encode_swap_in_amount_provided(
                                        token_from_address,
                                        token_to_address,
                                        U256::ZERO,
                                        swap_to,
                                        inside_call_payload,
                                    )?,
                                );

                                swap_opcode.set_call_stack(
                                    false,
                                    0,
                                    cur_pool.get_encoder().swap_in_amount_offset(token_from_address, token_to_address).unwrap(),
                                    0x20,
                                );
                                swap_opcode
                            }
                        }
                    } else {
                        trace!("uniswap v3 i != 0 else for pool={:?}", cur_pool.get_address());
                        let mut swap_opcode = MulticallerCall::new_call(
                            cur_pool.get_address(),
                            &cur_pool.get_encoder().encode_swap_in_amount_provided(
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
                            cur_pool.get_encoder().swap_in_amount_offset(token_from_address, token_to_address).unwrap(),
                            0x20,
                        );
                        swap_opcode
                    };

                    swap_opcode.set_return_stack(
                        true,
                        0,
                        cur_pool.get_encoder().swap_in_amount_return_offset(token_from_address, token_to_address).unwrap(),
                        0x20,
                    );

                    swap_opcodes.add(swap_opcode);

                    if next_pool.is_some() {
                        trace!("has next pool");
                        if let Some(x) = cur_pool.get_encoder().swap_in_amount_return_script(token_from_address, token_to_address) {
                            let calc_opcode = MulticallerCall::new_calculation_call(&x);
                            swap_opcodes.add(calc_opcode);
                        }
                    }
                }
                PoolClass::Curve => {
                    CurveSwapEncoder::encode_swap_in_amount_provided(
                        token_from_address,
                        token_to_address,
                        if i == 0 { swap_path.amount_in } else { SwapAmountType::RelativeStack(0) },
                        &mut swap_opcodes,
                        cur_pool,
                        next_pool,
                        self.multicaller,
                    )?;
                }
                PoolClass::LidoWstEth => {
                    WstEthSwapEncoder::encode_swap_in_amount_provided(
                        token_from_address,
                        token_to_address,
                        if i == 0 { swap_path.amount_in } else { SwapAmountType::RelativeStack(0) },
                        &mut swap_opcodes,
                        cur_pool,
                        next_pool,
                        self.multicaller,
                    )?;
                }

                PoolClass::LidoStEth => {
                    StEthSwapEncoder::encode_swap_in_amount_provided(
                        token_from_address,
                        token_to_address,
                        if i == 0 { swap_path.amount_in } else { SwapAmountType::RelativeStack(0) },
                        &mut swap_opcodes,
                        cur_pool,
                        next_pool,
                        self.multicaller,
                    )?;
                }
                _ => {
                    return Err(eyre!("POOL_TYPE_NOT_SUPPORTED"));
                }
            }
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
            EncoderHelper::encode_multicaller_transfer_tips_weth(min_balance, tips, to)
        } else {
            trace!("encode_multicaller_transfer_tips");
            EncoderHelper::encode_multicaller_transfer_tips(token_address, min_balance, tips, to)
        };
        tips_opcodes.add(MulticallerCall::new_internal_call(&call_data));
        Ok(tips_opcodes)
    }
}
