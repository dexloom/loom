use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use eyre::{eyre, Result};
use revm::InMemoryDB;
use revm::primitives::Env;

use defi_abi::uniswap_periphery::ICustomQuoter;
use loom_utils::evm::evm_call;

pub struct UniswapCustomQuoterEncoder {}

impl UniswapCustomQuoterEncoder {
    pub fn quote_exact_output_encode(pool: Address, token_in: Address, token_out: Address, fee: u32, amount_out: U256) -> Vec<u8> {
        let params = ICustomQuoter::QuoteExactOutputSingleParams {
            pool,
            tokenIn: token_in,
            tokenOut: token_out,
            amount: amount_out,
            fee: fee,
            sqrtPriceLimitX96: U256::ZERO,
        };
        let call = ICustomQuoter::quoteExactOutputSingleCall {
            params
        };
        call.abi_encode()
    }


    pub fn quote_exact_input_encode(pool: Address, token_in: Address, token_out: Address, fee: u32, amount_in: U256) -> Vec<u8> {
        let params = ICustomQuoter::QuoteExactInputSingleParams {
            pool,
            tokenIn: token_in,
            tokenOut: token_out,
            amountIn: amount_in,
            fee: fee,
            sqrtPriceLimitX96: U256::ZERO,
        };
        let call = ICustomQuoter::quoteExactInputSingleCall {
            params
        };
        call.abi_encode()
    }

    pub fn quote_exact_input_result_decode(data: &Vec<u8>) -> Result<U256> {
        let ret = ICustomQuoter::quoteExactInputSingleCall::abi_decode_returns(data, false);
        match ret {
            Ok(r) => {
                Ok(r.amountOut)
            }
            Err(e) => {
                Err(eyre!("CANNOT_DECODE_EXACT_INPUT_RETURN"))
            }
        }
    }
    pub fn quote_exact_output_result_decode(data: &Vec<u8>) -> Result<U256> {
        let ret = ICustomQuoter::quoteExactOutputSingleCall::abi_decode_returns(data, false);
        match ret {
            Ok(r) => {
                Ok(r.amountIn)
            }
            Err(e) => {
                Err(eyre!("CANNOT_DECODE_EXACT_INPUT_RETURN"))
            }
        }
    }
}


pub struct UniswapCustomQuoterStateReader {}

impl UniswapCustomQuoterStateReader {
    pub fn quote_exact_input(db: &InMemoryDB, env: Env, quoter_address: Address, pool: Address, token_from: Address, token_to: Address, fee: u32, amount: U256) -> eyre::Result<(U256, u64)> {
        let call_data_vec = UniswapCustomQuoterEncoder::quote_exact_input_encode(
            pool,
            token_from,
            token_to,
            fee,
            amount);

        let (value, gas_used) = evm_call(db, env, quoter_address, call_data_vec)?;

        let ret = UniswapCustomQuoterEncoder::quote_exact_input_result_decode(&value)?;
        Ok((ret, gas_used))
    }


    pub fn quote_exact_output(db: &InMemoryDB, env: Env, quoter_address: Address, pool: Address, token_from: Address, token_to: Address, fee: u32, amount: U256) -> eyre::Result<(U256, u64)> {
        let call_data_vec = UniswapCustomQuoterEncoder::quote_exact_output_encode(
            pool,
            token_from,
            token_to,
            fee,
            amount);

        let (value, gas_used) = evm_call(db, env, quoter_address, call_data_vec)?;

        let ret = UniswapCustomQuoterEncoder::quote_exact_output_result_decode(&value)?;
        Ok((ret, gas_used))
    }
}
