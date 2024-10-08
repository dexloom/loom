use std::convert::Infallible;

use alloy_primitives::aliases::U24;
use alloy_primitives::{Address, U160, U256};
use alloy_sol_types::SolCall;
use eyre::{eyre, Result};
use revm::primitives::Env;
use revm::DatabaseRef;
use tracing::error;

use defi_abi::uniswap_periphery::ICustomQuoter;
use loom_utils::evm::evm_call;

pub struct UniswapCustomQuoterEncoder {}

impl UniswapCustomQuoterEncoder {
    pub fn quote_exact_output_encode(pool: Address, token_in: Address, token_out: Address, fee: U24, amount_out: U256) -> Vec<u8> {
        let params = ICustomQuoter::QuoteExactOutputSingleParams {
            pool,
            tokenIn: token_in,
            tokenOut: token_out,
            amount: amount_out,
            fee,
            sqrtPriceLimitX96: U160::ZERO,
        };
        let call = ICustomQuoter::quoteExactOutputSingleCall { params };
        call.abi_encode()
    }

    pub fn quote_exact_input_encode(pool: Address, token_in: Address, token_out: Address, fee: U24, amount_in: U256) -> Vec<u8> {
        let params = ICustomQuoter::QuoteExactInputSingleParams {
            pool,
            tokenIn: token_in,
            tokenOut: token_out,
            amountIn: amount_in,
            fee,
            sqrtPriceLimitX96: U160::ZERO,
        };
        let call = ICustomQuoter::quoteExactInputSingleCall { params };
        call.abi_encode()
    }

    pub fn quote_exact_input_result_decode(data: &[u8]) -> Result<U256> {
        let ret = ICustomQuoter::quoteExactInputSingleCall::abi_decode_returns(data, false);
        match ret {
            Ok(r) => Ok(r.amountOut),
            Err(e) => {
                error!("Cannot decode exact input {}", e);
                Err(eyre!("CANNOT_DECODE_EXACT_INPUT_RETURN"))
            }
        }
    }
    pub fn quote_exact_output_result_decode(data: &[u8]) -> Result<U256> {
        let ret = ICustomQuoter::quoteExactOutputSingleCall::abi_decode_returns(data, false);
        match ret {
            Ok(r) => Ok(r.amountIn),
            Err(e) => {
                error!("Cannot decode exact input {}", e);
                Err(eyre!("CANNOT_DECODE_EXACT_INPUT_RETURN"))
            }
        }
    }
}

pub struct UniswapCustomQuoterStateReader {}

#[allow(clippy::too_many_arguments)]
impl UniswapCustomQuoterStateReader {
    pub fn quote_exact_input<DB: DatabaseRef<Error = Infallible>>(
        db: DB,
        env: Env,
        quoter_address: Address,
        pool: Address,
        token_from: Address,
        token_to: Address,
        fee: U24,
        amount: U256,
    ) -> Result<(U256, u64)> {
        let call_data_vec = UniswapCustomQuoterEncoder::quote_exact_input_encode(pool, token_from, token_to, fee, amount);

        let (value, gas_used) = evm_call(db, env, quoter_address, call_data_vec)?;

        let ret = UniswapCustomQuoterEncoder::quote_exact_input_result_decode(&value)?;
        Ok((ret, gas_used))
    }

    pub fn quote_exact_output<DB: DatabaseRef<Error = Infallible>>(
        db: DB,
        env: Env,
        quoter_address: Address,
        pool: Address,
        token_from: Address,
        token_to: Address,
        fee: U24,
        amount: U256,
    ) -> Result<(U256, u64)> {
        let call_data_vec = UniswapCustomQuoterEncoder::quote_exact_output_encode(pool, token_from, token_to, fee, amount);

        let (value, gas_used) = evm_call(db, env, quoter_address, call_data_vec)?;

        let ret = UniswapCustomQuoterEncoder::quote_exact_output_result_decode(&value)?;
        Ok((ret, gas_used))
    }
}
